//! Canister counts, the health they give and which mobs drop which heart: pure logic.

/// The four heart colours, in the order the amulet keeps their counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Colour {
    Red,
    Orange,
    Green,
    Blue,
}

impl Colour {
    pub const ALL: [Colour; 4] = [Colour::Red, Colour::Orange, Colour::Green, Colour::Blue];
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        match self {
            Colour::Red => "red",
            Colour::Orange => "orange",
            Colour::Green => "green",
            Colour::Blue => "blue",
        }
    }
}

/// Canisters of one colour an amulet holds (assumed 10, README "To check").
pub const MAX_PER_COLOUR: i32 = 10;
/// Health one canister adds: one heart.
pub const HEALTH_PER_CANISTER: f64 = 2.0;

/// The canisters in an amulet, by colour.
pub type Counts = [i32; 4];

/// The max health an amulet with `counts` adds.
pub fn bonus_health(counts: Counts) -> f64 {
    counts
        .iter()
        .map(|c| f64::from((*c).clamp(0, MAX_PER_COLOUR)))
        .sum::<f64>()
        * HEALTH_PER_CANISTER
}

/// `counts` with one more canister of `colour`, or none when that colour is full.
pub fn insert(counts: Counts, colour: Colour) -> Option<Counts> {
    let mut out = counts;
    let slot = &mut out[colour.index()];
    if *slot >= MAX_PER_COLOUR {
        return None;
    }
    *slot += 1;
    Some(out)
}

/// Which heart a mob may drop when a player kills it, and how likely (assumed chances).
pub fn heart_drop(kind: &str, hostile: bool) -> Option<(Colour, f32)> {
    match kind {
        "minecraft:ender_dragon" => Some((Colour::Green, 1.0)),
        "minecraft:wither" | "minecraft:elder_guardian" => Some((Colour::Orange, 1.0)),
        "minecraft:warden" => Some((Colour::Blue, 1.0)),
        _ if hostile => Some((Colour::Red, 0.05)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_canister_adds_a_heart_up_to_ten_per_colour() {
        assert_eq!(bonus_health([0; 4]), 0.0);
        assert_eq!(bonus_health([3, 0, 1, 0]), 8.0);
        assert_eq!(bonus_health([25, 10, 10, 10]), 80.0);
    }

    #[test]
    fn a_full_colour_refuses_another_canister() {
        assert_eq!(insert([0, 9, 0, 0], Colour::Orange), Some([0, 10, 0, 0]));
        assert_eq!(insert([0, 10, 0, 0], Colour::Orange), None);
        assert_eq!(insert([0, 10, 0, 0], Colour::Red), Some([1, 10, 0, 0]));
    }

    #[test]
    fn bosses_drop_their_own_heart_and_other_monsters_rarely_a_red_one() {
        assert_eq!(
            heart_drop("minecraft:ender_dragon", true),
            Some((Colour::Green, 1.0))
        );
        assert_eq!(
            heart_drop("minecraft:wither", true),
            Some((Colour::Orange, 1.0))
        );
        assert_eq!(
            heart_drop("minecraft:zombie", true),
            Some((Colour::Red, 0.05))
        );
        assert_eq!(heart_drop("minecraft:cow", false), None);
    }
}
