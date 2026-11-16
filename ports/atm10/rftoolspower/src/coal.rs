//! The Coal Generator: burns coal and charcoal (and coal blocks for ten times as long) into
//! RF, and pushes it to every block beside it.
use mod_api::{Compound, Tag};

/// RF made in each tick a fuel burns.
pub const RF_PER_TICK: i32 = 60;
/// Ticks one coal or charcoal burns.
pub const TICKS_PER_COAL: i32 = 600;
/// RF the generator holds.
pub const CAPACITY: i32 = 500_000;
/// RF it pushes out of each face, at most, in a tick.
pub const SEND_PER_TICK: i32 = 1_000;

/// Ticks one item of `id` burns, `None` for what the generator doesn't take. Other furnace
/// fuels burn in proportion to coal: their vanilla burn time × 600 / 1600.
pub fn burn_ticks(id: &str, vanilla_burn: Option<i32>) -> Option<i32> {
    match id {
        "minecraft:coal" | "minecraft:charcoal" => Some(TICKS_PER_COAL),
        "minecraft:coal_block" => Some(TICKS_PER_COAL * 10),
        _ => vanilla_burn
            .filter(|b| *b > 0)
            .map(|b| (b * TICKS_PER_COAL / 1_600).max(1)),
    }
}

/// When redstone lets a machine run, RFTools' three-way setting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RedstoneMode {
    /// Runs whatever the signal.
    #[default]
    Ignored,
    /// Runs only without a signal.
    Off,
    /// Runs only with a signal.
    On,
}

impl RedstoneMode {
    pub fn allows(self, powered: bool) -> bool {
        match self {
            RedstoneMode::Ignored => true,
            RedstoneMode::Off => !powered,
            RedstoneMode::On => powered,
        }
    }

    pub fn load(nbt: &Compound) -> Self {
        match nbt.get(&"rsMode".into()) {
            Some(Tag::Int(1)) => RedstoneMode::Off,
            Some(Tag::Int(2)) => RedstoneMode::On,
            _ => RedstoneMode::Ignored,
        }
    }
}

/// What a tick decided.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Step {
    /// Take one fuel item.
    pub consume: bool,
    /// RF made this tick.
    pub made: i32,
}

/// One tick: `burning` is the ticks the current fuel has left (kept in the compound), `fuel`
/// the burn ticks of the item that would be taken next. A new item is lit only when the buffer
/// has room for at least one tick's RF, so a full generator stops eating coal.
pub fn tick(burning: &mut i32, energy: i32, allowed: bool, fuel: Option<i32>) -> Step {
    let mut step = Step::default();
    if !allowed {
        return step;
    }
    if *burning <= 0 && energy + RF_PER_TICK <= CAPACITY {
        if let Some(t) = fuel {
            *burning = t;
            step.consume = true;
        }
    }
    if *burning > 0 {
        *burning -= 1;
        step.made = RF_PER_TICK.min(CAPACITY - energy);
    }
    step
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coal_burns_600_ticks_a_block_ten_times_that_and_a_log_in_proportion() {
        assert_eq!(burn_ticks("minecraft:charcoal", Some(1600)), Some(600));
        assert_eq!(burn_ticks("minecraft:coal_block", Some(16000)), Some(6000));
        assert_eq!(burn_ticks("minecraft:oak_log", Some(300)), Some(112));
        assert_eq!(burn_ticks("minecraft:stone", None), None);
    }

    #[test]
    fn one_coal_makes_36000_rf() {
        let (mut burning, mut energy, mut eaten) = (0, 0, 0);
        for _ in 0..700 {
            let s = tick(&mut burning, energy, true, Some(600).filter(|_| eaten == 0));
            eaten += i32::from(s.consume);
            energy += s.made;
        }
        assert_eq!((eaten, energy), (1, 36_000));
    }

    #[test]
    fn a_full_generator_lights_nothing_and_redstone_can_stop_it() {
        let mut burning = 0;
        assert_eq!(tick(&mut burning, CAPACITY, true, Some(600)), Step::default());
        assert_eq!(tick(&mut burning, 0, RedstoneMode::On.allows(false), Some(600)), Step::default());
        assert!(RedstoneMode::Off.allows(false) && !RedstoneMode::Off.allows(true));
    }
}
