//! The machines' numbers and state machines, apart from the game.

/// Slots every machine's container has (the hopper menu's five).
pub const SLOTS: usize = 5;
/// The crusher's second output slot.
pub const SECONDARY: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    CoalGenerator,
    PoweredFurnace,
    Crusher,
}

/// A generator's numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratorSpec {
    pub capacity: i32,
    pub per_tick: i32,
}

/// A processing machine's numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MachineSpec {
    /// FE drawn each tick while working.
    pub per_tick: i32,
    /// Ticks of power one operation needs.
    pub total: i32,
}

pub const COAL_GENERATOR: GeneratorSpec = GeneratorSpec { capacity: 60_000, per_tick: 30 };
pub const POWERED_FURNACE: MachineSpec = MachineSpec { per_tick: 25, total: 100 };
pub const CRUSHER: MachineSpec = MachineSpec { per_tick: 40, total: 100 };
/// Buffer of the processing machines.
pub const MACHINE_CAPACITY: i32 = 40_000;
/// Most a machine takes in one tick.
pub const MACHINE_INSERT: i32 = 1_000;

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::CoalGenerator, Kind::PoweredFurnace, Kind::Crusher];

    pub fn block_id(self) -> String {
        format!(
            "actuallyadditions:{}",
            match self {
                Kind::CoalGenerator => "coal_generator",
                Kind::PoweredFurnace => "powered_furnace",
                Kind::Crusher => "crusher",
            }
        )
    }

    pub fn of_block(id: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.block_id() == id)
    }

    pub fn machine(self) -> MachineSpec {
        match self {
            Kind::Crusher => CRUSHER,
            _ => POWERED_FURNACE,
        }
    }

    /// (capacity, max insert, max extract) of the energy buffer.
    pub fn energy(self) -> (i32, i32, i32) {
        match self {
            Kind::CoalGenerator => (COAL_GENERATOR.capacity, 0, COAL_GENERATOR.per_tick * 4),
            _ => (MACHINE_CAPACITY, MACHINE_INSERT, 0),
        }
    }

    /// The input slots and the output slot each one feeds.
    pub fn layout(self) -> (&'static [usize], &'static [usize]) {
        match self {
            Kind::CoalGenerator => (&[0], &[]),
            // two furnaces in one: slots 0 and 1 in, 2 and 3 out
            Kind::PoweredFurnace => (&[0, 1], &[2, 3]),
            // one input, the main result in slot 1, the chance result in slot 2
            Kind::Crusher => (&[0], &[1]),
        }
    }
}

/// A fuel's burning: ticks left and in all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Burn {
    pub left: i32,
    pub total: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BurnStep {
    pub energy: i32,
    pub consume_fuel: bool,
}

impl Burn {
    /// One tick of the coal generator: it burns only while its buffer has room for a tick's energy.
    pub fn tick(&mut self, stored: i32, capacity: i32, per_tick: i32, fuel_burn: Option<i32>) -> BurnStep {
        let mut step = BurnStep::default();
        if capacity - stored < per_tick {
            return step;
        }
        if self.left <= 0 {
            if let Some(b) = fuel_burn {
                self.left = b;
                self.total = b;
                step.consume_fuel = true;
            }
        }
        if self.left > 0 {
            self.left -= 1;
            step.energy = per_tick;
        }
        step
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorkStep {
    pub energy_used: i32,
    pub done: bool,
}

/// One tick of a processing machine: with work and enough energy it advances; without work its
/// progress resets; without energy it holds.
pub fn work_step(progress: &mut i32, stored: i32, spec: MachineSpec, has_work: bool) -> WorkStep {
    if !has_work {
        *progress = 0;
        return WorkStep::default();
    }
    if stored < spec.per_tick {
        return WorkStep::default();
    }
    *progress += 1;
    let done = *progress >= spec.total;
    if done {
        *progress = 0;
    }
    WorkStep { energy_used: spec.per_tick, done }
}

/// What the crusher makes of an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crushing {
    pub primary: (&'static str, u32),
    /// Item, count and the chance in percent.
    pub secondary: Option<(&'static str, u32, u32)>,
}

/// The crusher's recipes with vanilla items only (the original's are data-driven and mostly make
/// the dusts of other mods).
pub fn crush(input: &str) -> Option<Crushing> {
    Some(match input {
        "minecraft:cobblestone" => Crushing { primary: ("minecraft:gravel", 1), secondary: Some(("minecraft:flint", 1, 10)) },
        "minecraft:gravel" => Crushing { primary: ("minecraft:sand", 1), secondary: None },
        "minecraft:stone" => Crushing { primary: ("minecraft:cobblestone", 1), secondary: None },
        "minecraft:bone" => Crushing { primary: ("minecraft:bone_meal", 6), secondary: None },
        "minecraft:blaze_rod" => Crushing { primary: ("minecraft:blaze_powder", 4), secondary: None },
        _ => return None,
    })
}

/// A percent roll from the game time and position, the same on every run.
pub fn roll(time: i64, pos: [i32; 3]) -> u32 {
    let mut h = (time as u64) ^ ((pos[0] as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    h ^= (pos[1] as i64 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= (pos[2] as i64 as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 32;
    (h % 100) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_coal_item_of_1600_ticks_makes_48000_fe() {
        let mut burn = Burn::default();
        let mut made = 0;
        let mut fuels = 0;
        for _ in 0..1600 {
            let s = burn.tick(0, 60_000, 30, Some(1600));
            made += s.energy;
            fuels += i32::from(s.consume_fuel);
        }
        assert_eq!((made, fuels), (48_000, 1));
        assert_eq!(burn.left, 0);
    }

    #[test]
    fn a_full_generator_holds_its_fuel() {
        let mut burn = Burn::default();
        let s = burn.tick(59_990, 60_000, 30, Some(1600));
        assert_eq!(s, BurnStep::default());
        assert_eq!(burn.left, 0);
        // already burning: the burn pauses too
        let mut burn = Burn { left: 500, total: 1600 };
        burn.tick(60_000, 60_000, 30, None);
        assert_eq!(burn.left, 500);
    }

    #[test]
    fn a_powered_furnace_needs_100_ticks_of_25_fe() {
        let mut progress = 0;
        let mut stored = 10_000;
        let mut ticks = 0;
        loop {
            ticks += 1;
            let s = work_step(&mut progress, stored, POWERED_FURNACE, true);
            stored -= s.energy_used;
            if s.done {
                break;
            }
        }
        assert_eq!((ticks, stored), (100, 10_000 - 2_500));
    }

    #[test]
    fn without_energy_progress_holds_and_without_work_it_resets() {
        let mut progress = 40;
        let s = work_step(&mut progress, 39, CRUSHER, true);
        assert_eq!((progress, s.energy_used), (40, 0));
        work_step(&mut progress, 10_000, CRUSHER, false);
        assert_eq!(progress, 0);
    }

    #[test]
    fn the_chance_roll_is_a_percent_and_spreads() {
        let rolls: Vec<u32> = (0..2000).map(|t| roll(t, [3, 64, -7])).collect();
        assert!(rolls.iter().all(|r| *r < 100));
        let low = rolls.iter().filter(|r| **r < 10).count();
        assert!((120..=280).contains(&low), "{low}");
    }
}
