//! Dimensional Cells: batteries in three tiers whose six faces are each set to none, input or
//! output, and which share their energy with the cells they touch, so a block of cells acts as
//! one big battery.
use mod_api::{Compound, Direction, Tag};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Simple,
    Normal,
    Advanced,
}

/// RF a normal cell holds (RFTools Power's default `rfPerNormalCell`).
pub const NORMAL_CAPACITY: i32 = 1_000_000;
/// RF a normal cell moves through one face in one tick.
pub const NORMAL_RF_PER_TICK: i32 = 5_000;

impl Tier {
    pub const ALL: [Tier; 3] = [Tier::Simple, Tier::Normal, Tier::Advanced];

    /// The block's path in the `rftoolspower` namespace.
    pub fn block(self) -> &'static str {
        match self {
            Tier::Normal => "cell1",
            Tier::Advanced => "cell2",
            Tier::Simple => "cell3",
        }
    }

    /// The tier's size against a normal cell, in quarters (simple ¼, advanced ×4).
    fn quarters(self) -> i32 {
        match self {
            Tier::Simple => 1,
            Tier::Normal => 4,
            Tier::Advanced => 16,
        }
    }

    pub fn capacity(self) -> i32 {
        NORMAL_CAPACITY / 4 * self.quarters()
    }

    pub fn rf_per_tick(self) -> i32 {
        NORMAL_RF_PER_TICK / 4 * self.quarters()
    }
}

/// What one face of a cell does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SideMode {
    /// Not connected: nothing goes in or out.
    #[default]
    None,
    /// Takes energy in; never gives it.
    Input,
    /// Pushes energy out every tick, and lets it be drawn.
    Output,
}

impl SideMode {
    /// The next mode, as a click on the face cycles them.
    pub fn next(self) -> Self {
        match self {
            SideMode::None => SideMode::Input,
            SideMode::Input => SideMode::Output,
            SideMode::Output => SideMode::None,
        }
    }

    pub fn takes(self) -> bool {
        self == SideMode::Input
    }

    pub fn gives(self) -> bool {
        self == SideMode::Output
    }

    fn code(self) -> i32 {
        self as i32
    }

    fn from_code(c: i32) -> Self {
        match c {
            1 => SideMode::Input,
            2 => SideMode::Output,
            _ => SideMode::None,
        }
    }
}

/// The six faces' modes, in `Direction` order, saved as an int array `sides`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sides(pub [SideMode; 6]);

impl Sides {
    pub fn get(&self, dir: Direction) -> SideMode {
        self.0[dir as usize]
    }

    pub fn cycle(&mut self, dir: Direction) -> SideMode {
        let next = self.get(dir).next();
        self.0[dir as usize] = next;
        next
    }

    pub fn outputs(&self) -> Vec<Direction> {
        Direction::ALL.into_iter().filter(|d| self.get(*d).gives()).collect()
    }

    pub fn save(&self, nbt: &mut Compound) {
        nbt.insert("sides".into(), Tag::IntArray(self.0.iter().map(|m| m.code()).collect()));
    }

    pub fn load(nbt: &Compound) -> Self {
        let mut sides = Sides::default();
        if let Some(Tag::IntArray(codes)) = nbt.get(&"sides".into()) {
            for (slot, code) in sides.0.iter_mut().zip(codes) {
                *slot = SideMode::from_code(*code);
            }
        }
        sides
    }
}

/// How much should move from cell `a` to touching cell `b` (negative: from `b` to `a`) so that
/// both are filled to the same fraction, no more than `limit` in one tick. Cells of a block
/// share this way until they level out, which is how a group acts as one battery.
pub fn level_out(a: i64, cap_a: i64, b: i64, cap_b: i64, limit: i64) -> i64 {
    if cap_a <= 0 || cap_b <= 0 {
        return 0;
    }
    // After the move a - x over cap_a equals b + x over cap_b.
    let x = (a * cap_b - b * cap_a) / (cap_a + cap_b);
    x.clamp(-limit, limit)
}

/// The face of the block at `pos` a click at `hit` (`BlockHit.face`, a unit vector) is on.
pub fn face_of(normal: [i32; 3]) -> Option<Direction> {
    Direction::ALL.into_iter().find(|d| d.normal() == normal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_hold_a_quarter_one_and_four_million() {
        assert_eq!(Tier::Simple.capacity(), 250_000);
        assert_eq!(Tier::Normal.capacity(), 1_000_000);
        assert_eq!(Tier::Advanced.capacity(), 4_000_000);
        assert_eq!(Tier::Advanced.rf_per_tick(), 20_000);
    }

    #[test]
    fn a_face_cycles_none_input_output_and_only_outputs_push() {
        let mut sides = Sides::default();
        assert_eq!(sides.cycle(Direction::Up), SideMode::Input);
        assert_eq!(sides.cycle(Direction::Up), SideMode::Output);
        sides.cycle(Direction::North);
        assert_eq!(sides.outputs(), vec![Direction::Up]);
        assert_eq!(sides.cycle(Direction::Up), SideMode::None);
        assert!(sides.outputs().is_empty());
    }

    #[test]
    fn faces_round_trip_through_their_saved_form() {
        let sides = Sides([SideMode::Output, SideMode::None, SideMode::Input, SideMode::Input, SideMode::None, SideMode::Output]);
        let mut nbt = Compound::new();
        sides.save(&mut nbt);
        assert_eq!(Sides::load(&nbt), sides);
    }

    #[test]
    fn touching_cells_level_out_to_the_same_fraction() {
        // A full normal cell beside an empty advanced one: 1M over 5M total, 20% each.
        let x = level_out(1_000_000, 1_000_000, 0, 4_000_000, i64::MAX);
        assert_eq!(x, 800_000);
        assert_eq!((1_000_000 - x) * 5, 1_000_000);
        // The other way round moves the other way, and the limit holds.
        assert_eq!(level_out(0, 1_000_000, 4_000_000, 4_000_000, 5_000), -5_000);
        assert_eq!(level_out(500, 1_000, 500, 1_000, 5_000), 0);
    }

    #[test]
    fn a_click_finds_its_face() {
        assert_eq!(face_of([0, 1, 0]), Some(Direction::Up));
        assert_eq!(face_of([-1, 0, 0]), Some(Direction::West));
        assert_eq!(face_of([1, 1, 0]), None);
    }
}
