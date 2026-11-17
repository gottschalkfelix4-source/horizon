//! Data models, tiers and the two machines' numbers, apart from the game.

/// How far a data model has learned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Faulty,
    Basic,
    Advanced,
    Superior,
    SelfAware,
}

/// Data a model needs to reach each tier, from `Faulty` up.
pub const THRESHOLDS: [u32; 5] = [0, 6, 24, 100, 400];

impl Tier {
    pub fn of(data: u32) -> Tier {
        match THRESHOLDS.iter().rposition(|t| data >= *t).unwrap_or(0) {
            0 => Tier::Faulty,
            1 => Tier::Basic,
            2 => Tier::Advanced,
            3 => Tier::Superior,
            _ => Tier::SelfAware,
        }
    }

    /// Whether a model of this tier may be simulated.
    pub fn simulates(self) -> bool {
        self >= Tier::Basic
    }
}

/// Data a model has after one more kill: it stops growing at the last tier's threshold.
pub fn learn(data: u32, per_kill: u32) -> u32 {
    (data + per_kill).min(THRESHOLDS[4])
}

/// One mob the mod knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mob {
    pub name: &'static str,
    pub entity: &'static str,
    /// What the loot fabricator makes of one prediction.
    pub loot: (&'static str, u32),
}

pub const MOBS: [Mob; 3] = [
    Mob { name: "zombie", entity: "minecraft:zombie", loot: ("minecraft:rotten_flesh", 4) },
    Mob { name: "skeleton", entity: "minecraft:skeleton", loot: ("minecraft:bone", 4) },
    Mob { name: "creeper", entity: "minecraft:creeper", loot: ("minecraft:gunpowder", 4) },
];

pub fn mob_of_entity(entity: &str) -> Option<&'static Mob> {
    MOBS.iter().find(|m| m.entity == entity)
}

/// The mob a `hostilenetworks:data_model_<mob>` or `prediction_<mob>` item is of.
pub fn mob_of_item(item: &str, prefix: &str) -> Option<&'static Mob> {
    let name = item.strip_prefix("hostilenetworks:")?.strip_prefix(prefix)?;
    MOBS.iter().find(|m| m.name == name)
}

/// A machine's numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    pub per_tick: i32,
    pub total: i32,
}

pub const SIM_CHAMBER: Spec = Spec { per_tick: 256, total: 300 };
pub const LOOT_FABRICATOR: Spec = Spec { per_tick: 256, total: 100 };
pub const BUFFER: i32 = 200_000;
pub const INSERT: i32 = 4_000;

/// One tick of a machine: advances with work and energy, holds without energy, resets without work.
/// Returns (energy used, finished).
pub fn step(progress: &mut i32, stored: i32, spec: Spec, has_work: bool) -> (i32, bool) {
    if !has_work {
        *progress = 0;
        return (0, false);
    }
    if stored < spec.per_tick {
        return (0, false);
    }
    *progress += 1;
    let done = *progress >= spec.total;
    if done {
        *progress = 0;
    }
    (spec.per_tick, done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_follow_the_data_a_model_gathered() {
        assert_eq!(Tier::of(0), Tier::Faulty);
        assert_eq!(Tier::of(5), Tier::Faulty);
        assert_eq!(Tier::of(6), Tier::Basic);
        assert_eq!(Tier::of(99), Tier::Advanced);
        assert_eq!(Tier::of(100), Tier::Superior);
        assert_eq!(Tier::of(10_000), Tier::SelfAware);
    }

    #[test]
    fn only_a_basic_model_or_better_may_be_simulated() {
        assert!(!Tier::Faulty.simulates());
        assert!(Tier::Basic.simulates());
        assert!(Tier::SelfAware.simulates());
    }

    #[test]
    fn learning_stops_at_the_last_tier() {
        assert_eq!(learn(5, 1), 6);
        assert_eq!(learn(399, 1), 400);
        assert_eq!(learn(400, 1), 400);
    }

    #[test]
    fn a_simulation_takes_300_ticks_of_256_fe_and_stalls_without_energy() {
        let (mut p, mut stored, mut used) = (0, 100_000, 0);
        let mut ticks = 0;
        loop {
            ticks += 1;
            let (e, done) = step(&mut p, stored, SIM_CHAMBER, true);
            stored -= e;
            used += e;
            if done {
                break;
            }
        }
        assert_eq!((ticks, used), (300, 300 * 256));
        let (e, done) = step(&mut p, 255, SIM_CHAMBER, true);
        assert_eq!((e, done, p), (0, false, 0));
    }

    #[test]
    fn items_and_entities_name_the_same_mob() {
        assert_eq!(mob_of_item("hostilenetworks:data_model_creeper", "data_model_").unwrap().entity, "minecraft:creeper");
        assert_eq!(mob_of_item("hostilenetworks:prediction_zombie", "prediction_").unwrap().loot.0, "minecraft:rotten_flesh");
        assert!(mob_of_item("hostilenetworks:data_model_zombie", "prediction_").is_none());
        assert!(mob_of_entity("minecraft:cow").is_none());
    }
}
