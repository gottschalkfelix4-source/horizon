//! Spawner stats and the modifiers that change them: pure logic, no game access.
//!
//! The defaults follow Apothic Spawners' public description (an ATM guide's table): which item
//! changes which stat and by how much. The caps are assumed and listed in README.md to check.

use mod_api::{Compound, Tag};

/// A spawner's numbers, as vanilla's `BaseSpawner` saves them (all `short`s in its NBT).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub min_delay: i32,
    pub max_delay: i32,
    pub spawn_count: i32,
    pub max_nearby: i32,
    pub player_range: i32,
    pub spawn_range: i32,
}

/// Vanilla's `BaseSpawner` defaults.
impl Default for Stats {
    fn default() -> Self {
        Self {
            min_delay: 200,
            max_delay: 800,
            spawn_count: 4,
            max_nearby: 6,
            player_range: 16,
            spawn_range: 4,
        }
    }
}

const KEYS: [&str; 6] = [
    "MinSpawnDelay",
    "MaxSpawnDelay",
    "SpawnCount",
    "MaxNearbyEntities",
    "RequiredPlayerRange",
    "SpawnRange",
];

fn short(data: &Compound, key: &str) -> Option<i32> {
    match data.get(&key.into())? {
        Tag::Short(v) => Some(i32::from(*v)),
        Tag::Int(v) => Some(*v),
        Tag::Byte(v) => Some(i32::from(*v)),
        _ => None,
    }
}

impl Stats {
    /// Reads the stats from a spawner's saved data; a missing field keeps vanilla's default.
    pub fn from_nbt(data: &Compound) -> Self {
        let d = Self::default();
        let get = |i: usize, default: i32| short(data, KEYS[i]).unwrap_or(default);
        Self {
            min_delay: get(0, d.min_delay),
            max_delay: get(1, d.max_delay),
            spawn_count: get(2, d.spawn_count),
            max_nearby: get(3, d.max_nearby),
            player_range: get(4, d.player_range),
            spawn_range: get(5, d.spawn_range),
        }
    }

    /// The data to merge into the spawner to store these stats.
    pub fn to_nbt(self) -> Compound {
        let values = [
            self.min_delay,
            self.max_delay,
            self.spawn_count,
            self.max_nearby,
            self.player_range,
            self.spawn_range,
        ];
        KEYS.iter()
            .zip(values)
            .map(|(k, v)| {
                (
                    (*k).into(),
                    Tag::Short(v.clamp(0, i32::from(i16::MAX)) as i16),
                )
            })
            .collect()
    }

    fn get(&self, stat: Stat) -> i32 {
        match stat {
            Stat::MinDelay => self.min_delay,
            Stat::MaxDelay => self.max_delay,
            Stat::SpawnCount => self.spawn_count,
            Stat::MaxNearby => self.max_nearby,
            Stat::PlayerRange => self.player_range,
            Stat::SpawnRange => self.spawn_range,
        }
    }

    fn set(&mut self, stat: Stat, value: i32) {
        match stat {
            Stat::MinDelay => self.min_delay = value,
            Stat::MaxDelay => self.max_delay = value,
            Stat::SpawnCount => self.spawn_count = value,
            Stat::MaxNearby => self.max_nearby = value,
            Stat::PlayerRange => self.player_range = value,
            Stat::SpawnRange => self.spawn_range = value,
        }
    }
}

/// A numeric stat a modifier changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stat {
    MinDelay,
    MaxDelay,
    SpawnCount,
    MaxNearby,
    PlayerRange,
    SpawnRange,
}

impl Stat {
    /// The range the stat stays in (assumed caps, README "To check").
    pub fn bounds(self) -> (i32, i32) {
        match self {
            Stat::MinDelay | Stat::MaxDelay => (20, i32::from(i16::MAX)),
            Stat::SpawnCount => (1, 16),
            Stat::MaxNearby => (1, 32),
            Stat::PlayerRange => (1, 64),
            Stat::SpawnRange => (1, 32),
        }
    }
}

/// A behaviour switch kept beside the spawner (vanilla's spawner has no field for it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flag {
    IgnorePlayers,
    IgnoreLight,
    IgnoreConditions,
    RedstoneControl,
    Echoing,
    Burning,
    Youthful,
    Silent,
    NoAi,
}

impl Flag {
    pub const ALL: [Flag; 9] = [
        Flag::IgnorePlayers,
        Flag::IgnoreLight,
        Flag::IgnoreConditions,
        Flag::RedstoneControl,
        Flag::Echoing,
        Flag::Burning,
        Flag::Youthful,
        Flag::Silent,
        Flag::NoAi,
    ];
    pub fn bit(self) -> u16 {
        1 << Flag::ALL.iter().position(|f| *f == self).unwrap_or(0)
    }
    /// The entity tag a spawned mob carries while this flag still has to be applied to it.
    pub fn mob_tag(self) -> &'static str {
        match self {
            Flag::IgnorePlayers => "apothic_spawners.ignore_players",
            Flag::IgnoreLight => "apothic_spawners.ignore_light",
            Flag::IgnoreConditions => "apothic_spawners.ignore_conditions",
            Flag::RedstoneControl => "apothic_spawners.redstone_control",
            Flag::Echoing => "apothic_spawners.echoing",
            Flag::Burning => "apothic_spawners.burning",
            Flag::Youthful => "apothic_spawners.youthful",
            Flag::Silent => "apothic_spawners.silent",
            Flag::NoAi => "apothic_spawners.no_ai",
        }
    }
}

/// What the mod keeps beside one spawner: its switches and the initial-health change.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Extras {
    pub flags: u16,
    /// Percent added to a spawned mob's max health, negative from Pointed Dripstone.
    pub health_percent: i32,
}

impl Extras {
    pub fn has(self, flag: Flag) -> bool {
        self.flags & flag.bit() != 0
    }
    /// Initial health is assumed to stay within -95..=0 percent.
    pub const HEALTH_BOUNDS: (i32, i32) = (-95, 0);
}

/// One change a modifier makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Add(Stat, i32),
    Set(Flag),
    HealthPercent(i32),
}

/// A spawner modifier: the item in the main hand and what it changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Modifier {
    pub item: &'static str,
    pub change: Change,
}

/// The item in the off hand that inverts a modifier.
pub const INVERTER: &str = "minecraft:quartz";

/// The default modifiers, from Apothic Spawners' public description.
pub const MODIFIERS: &[Modifier] = &[
    Modifier {
        item: "minecraft:sugar",
        change: Change::Add(Stat::MinDelay, -10),
    },
    Modifier {
        item: "minecraft:clock",
        change: Change::Add(Stat::MaxDelay, -20),
    },
    Modifier {
        item: "minecraft:fermented_spider_eye",
        change: Change::Add(Stat::SpawnCount, 2),
    },
    Modifier {
        item: "minecraft:ghast_tear",
        change: Change::Add(Stat::MaxNearby, 2),
    },
    Modifier {
        item: "minecraft:prismarine_crystals",
        change: Change::Add(Stat::PlayerRange, 4),
    },
    Modifier {
        item: "minecraft:piston",
        change: Change::Add(Stat::SpawnRange, 2),
    },
    Modifier {
        item: "minecraft:nether_star",
        change: Change::Set(Flag::IgnorePlayers),
    },
    Modifier {
        item: "minecraft:soul_lantern",
        change: Change::Set(Flag::IgnoreLight),
    },
    Modifier {
        item: "minecraft:conduit",
        change: Change::Set(Flag::IgnoreConditions),
    },
    Modifier {
        item: "minecraft:comparator",
        change: Change::Set(Flag::RedstoneControl),
    },
    Modifier {
        item: "minecraft:echo_shard",
        change: Change::Set(Flag::Echoing),
    },
    Modifier {
        item: "minecraft:campfire",
        change: Change::Set(Flag::Burning),
    },
    Modifier {
        item: "minecraft:turtle_egg",
        change: Change::Set(Flag::Youthful),
    },
    Modifier {
        item: "minecraft:white_wool",
        change: Change::Set(Flag::Silent),
    },
    Modifier {
        item: "minecraft:chorus_fruit",
        change: Change::Set(Flag::NoAi),
    },
    Modifier {
        item: "minecraft:pointed_dripstone",
        change: Change::HealthPercent(-5),
    },
];

/// The modifier for `item`, if any.
pub fn modifier_for(item: &str) -> Option<&'static Modifier> {
    MODIFIERS.iter().find(|m| m.item == item)
}

/// A spawner's whole state as a modifier sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Spawner {
    pub stats: Stats,
    pub extras: Extras,
}

/// Applies `change` (inverted with Quartz in the off hand). `None` when nothing would change:
/// the item is then not used up. Delays keep `min <= max`.
pub fn apply(spawner: Spawner, change: Change, inverted: bool) -> Option<Spawner> {
    let mut out = spawner;
    match change {
        Change::Add(stat, amount) => {
            let amount = if inverted { -amount } else { amount };
            let (lo, mut hi) = stat.bounds();
            let mut lo = lo;
            match stat {
                Stat::MinDelay => hi = hi.min(spawner.stats.max_delay),
                Stat::MaxDelay => lo = lo.max(spawner.stats.min_delay),
                _ => {}
            }
            let now = spawner.stats.get(stat);
            let next = (now + amount).clamp(lo, hi.max(lo));
            out.stats.set(stat, next);
        }
        Change::Set(flag) => {
            if inverted {
                out.extras.flags &= !flag.bit();
            } else {
                out.extras.flags |= flag.bit();
            }
        }
        Change::HealthPercent(amount) => {
            let amount = if inverted { -amount } else { amount };
            let (lo, hi) = Extras::HEALTH_BOUNDS;
            out.extras.health_percent = (spawner.extras.health_percent + amount).clamp(lo, hi);
        }
    }
    (out != spawner).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sugar_lowers_the_minimum_delay_and_quartz_raises_it() {
        let s = Spawner::default();
        let sugar = modifier_for("minecraft:sugar").unwrap().change;
        assert_eq!(apply(s, sugar, false).unwrap().stats.min_delay, 190);
        assert_eq!(apply(s, sugar, true).unwrap().stats.min_delay, 210);
    }

    #[test]
    fn a_stat_at_its_cap_refuses_the_item() {
        let mut s = Spawner::default();
        s.stats.spawn_count = 16;
        let eye = modifier_for("minecraft:fermented_spider_eye")
            .unwrap()
            .change;
        assert_eq!(apply(s, eye, false), None);
        s.stats.spawn_count = 15;
        assert_eq!(apply(s, eye, false).unwrap().stats.spawn_count, 16);
    }

    #[test]
    fn the_minimum_delay_never_passes_the_maximum() {
        let mut s = Spawner::default();
        s.stats.min_delay = 300;
        s.stats.max_delay = 305;
        let clock = modifier_for("minecraft:clock").unwrap().change;
        assert_eq!(apply(s, clock, false).unwrap().stats.max_delay, 300);
        let sugar = modifier_for("minecraft:sugar").unwrap().change;
        assert_eq!(apply(s, sugar, true).unwrap().stats.min_delay, 305);
    }

    #[test]
    fn a_switch_turns_on_and_quartz_turns_it_off() {
        let s = Spawner::default();
        let star = modifier_for("minecraft:nether_star").unwrap().change;
        let on = apply(s, star, false).unwrap();
        assert!(on.extras.has(Flag::IgnorePlayers));
        assert!(!on.extras.has(Flag::Silent));
        assert_eq!(apply(on, star, false), None);
        assert!(
            !apply(on, star, true)
                .unwrap()
                .extras
                .has(Flag::IgnorePlayers)
        );
    }

    #[test]
    fn stats_round_trip_through_spawner_nbt_and_default_when_missing() {
        let stats = Stats {
            min_delay: 40,
            max_delay: 120,
            spawn_count: 9,
            max_nearby: 20,
            player_range: 32,
            spawn_range: 6,
        };
        assert_eq!(Stats::from_nbt(&stats.to_nbt()), stats);
        assert_eq!(Stats::from_nbt(&Compound::new()), Stats::default());
    }

    #[test]
    fn dripstone_lowers_initial_health_down_to_its_floor() {
        let mut s = Spawner::default();
        let drip = modifier_for("minecraft:pointed_dripstone").unwrap().change;
        assert_eq!(apply(s, drip, false).unwrap().extras.health_percent, -5);
        s.extras.health_percent = -95;
        assert_eq!(apply(s, drip, false), None);
    }
}
