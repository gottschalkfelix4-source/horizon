//! Gateways to Eternity for Horizon, first slice: a Gate Pearl opens a gateway that spawns waves
//! of mobs; kill every wave in time and inside the leash range to win its rewards.
//!
//! Written from the mod's public description and from how gateways play, never from its code
//! (docs/modpacks.md, "Licences"). The two built-in gateways below are approximations from memory
//! of the mod's own data, listed in README.md to check against the original.
//!
//! How it works:
//! - `gateways:gate_pearl` used on a block opens the gateway its stack names
//!   (`minecraft:custom_data` `{gateway: "gateways:basic/blaze"}`, or the first built-in one) on
//!   the face clicked, and is used up.
//! - The gateway's state machine ([`Gateway::step`]) runs after each tick of its level: a setup
//!   pause, then a wave of mobs tagged `gateways.wave_mob`; once they are all dead the wave's
//!   rewards drop and the next wave's setup starts. A wave that runs out of time, or a mob that
//!   strays past the leash range, fails the gateway and removes what is left of the wave.
//! - A boss bar shows the wave and how many of its mobs are left, to the player who opened it.
use std::sync::{Mutex, OnceLock};

use mod_api::{
    BossBarRef, Compound, HostSide, InteractionResult, ItemPart, LevelTickPostEvent, PlayerRef,
    Registrar, RegistrationContext, ServerHost, Tag, Text, UseOnEvent,
};

/// One wave of a gateway.
#[derive(Debug)]
pub struct Wave {
    /// Entity types and how many of each.
    pub entities: &'static [(&'static str, u32)],
    /// Items dropped when the wave is cleared.
    pub rewards: &'static [(&'static str, u32)],
    /// Ticks the wave may last before the gateway fails.
    pub max_wave_time: i64,
    /// Ticks between the previous wave (or the opening) and this one.
    pub setup_time: i64,
}

impl Wave {
    pub fn size(&self) -> u32 {
        self.entities.iter().map(|(_, n)| n).sum()
    }
}

/// A gateway's definition.
#[derive(Debug)]
pub struct Definition {
    pub id: &'static str,
    /// How far a wave's mob may go from the gateway.
    pub leash_range: f64,
    /// How far from the gateway its mobs appear.
    pub spawn_range: f64,
    pub waves: &'static [Wave],
}

/// The built-in gateways (approximations from memory of the mod's data; see README.md).
pub static DEFINITIONS: &[Definition] = &[
    Definition {
        id: "gateways:basic/blaze",
        leash_range: 32.0,
        spawn_range: 3.0,
        waves: &[
            Wave {
                entities: &[("minecraft:blaze", 3)],
                rewards: &[("minecraft:blaze_rod", 4)],
                max_wave_time: 600,
                setup_time: 60,
            },
            Wave {
                entities: &[("minecraft:blaze", 5)],
                rewards: &[("minecraft:blaze_rod", 8)],
                max_wave_time: 800,
                setup_time: 100,
            },
            Wave {
                entities: &[("minecraft:blaze", 7)],
                rewards: &[
                    ("minecraft:blaze_rod", 16),
                    ("minecraft:experience_bottle", 8),
                ],
                max_wave_time: 1000,
                setup_time: 100,
            },
        ],
    },
    Definition {
        id: "gateways:basic/slime",
        leash_range: 32.0,
        spawn_range: 3.0,
        waves: &[
            Wave {
                entities: &[("minecraft:slime", 4)],
                rewards: &[("minecraft:slime_ball", 8)],
                max_wave_time: 600,
                setup_time: 60,
            },
            Wave {
                entities: &[("minecraft:slime", 6)],
                rewards: &[
                    ("minecraft:slime_ball", 16),
                    ("minecraft:experience_bottle", 4),
                ],
                max_wave_time: 800,
                setup_time: 100,
            },
        ],
    },
];

pub fn definition(id: &str) -> Option<&'static Definition> {
    DEFINITIONS.iter().find(|d| d.id == id)
}

/// Where a gateway is in its run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Waiting to spawn wave `wave` at tick `until`.
    Setup { wave: usize, until: i64 },
    /// Wave `wave` is out since tick `since`.
    Wave { wave: usize, since: i64 },
    /// Finished, won or lost.
    Closed,
}

/// What the game must do after a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    SpawnWave(usize),
    DropRewards(usize),
    Fail(Failure),
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The wave ran out of time.
    Timeout,
    /// A wave's mob went past the leash range.
    LeftLeash,
}

/// A wave's mobs as the game saw them this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaveView {
    pub alive: usize,
    pub out_of_leash: bool,
}

/// An open gateway's run.
#[derive(Debug)]
pub struct Gateway {
    pub def: &'static Definition,
    pub phase: Phase,
}

impl Gateway {
    pub fn open(def: &'static Definition, now: i64) -> Self {
        let until = now + def.waves.first().map_or(0, |w| w.setup_time);
        Self {
            def,
            phase: Phase::Setup { wave: 0, until },
        }
    }

    /// Advance the run to tick `now`, given what is left of the current wave.
    pub fn step(&mut self, now: i64, view: WaveView) -> Vec<Action> {
        match self.phase {
            Phase::Closed => Vec::new(),
            Phase::Setup { wave, until } => {
                if now < until {
                    return Vec::new();
                }
                self.phase = Phase::Wave { wave, since: now };
                vec![Action::SpawnWave(wave)]
            }
            Phase::Wave { wave, since } => {
                if view.out_of_leash {
                    self.phase = Phase::Closed;
                    return vec![Action::Fail(Failure::LeftLeash)];
                }
                if view.alive > 0 {
                    if now - since >= self.def.waves[wave].max_wave_time {
                        self.phase = Phase::Closed;
                        return vec![Action::Fail(Failure::Timeout)];
                    }
                    return Vec::new();
                }
                let next = wave + 1;
                match self.def.waves.get(next) {
                    Some(w) => {
                        self.phase = Phase::Setup {
                            wave: next,
                            until: now + w.setup_time,
                        };
                        vec![Action::DropRewards(wave)]
                    }
                    None => {
                        self.phase = Phase::Closed;
                        vec![Action::DropRewards(wave), Action::Complete]
                    }
                }
            }
        }
    }
}

/// The `i`th of `n` spawn points on a circle of `radius` around `center`.
pub fn spawn_point(center: [f64; 3], i: u32, n: u32, radius: f64) -> [f64; 3] {
    let angle = std::f64::consts::TAU * f64::from(i) / f64::from(n.max(1));
    [
        center[0] + radius * angle.cos(),
        center[1],
        center[2] + radius * angle.sin(),
    ]
}

const MOB_TAG: &str = "gateways.wave_mob";
const DEFAULT: &str = "gateways:basic/blaze";

/// An open gateway in the world.
struct Open {
    run: Gateway,
    dimension: String,
    center: [f64; 3],
    mobs: Vec<u32>,
    bar: Option<BossBarRef>,
    opener: Option<PlayerRef>,
}

fn gateways() -> std::sync::MutexGuard<'static, Vec<Open>> {
    static OPEN: OnceLock<Mutex<Vec<Open>>> = OnceLock::new();
    OPEN.get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn string(value: &str) -> Tag {
    Tag::String(value.into())
}

/// The gateway a pearl names: its `minecraft:custom_data` `gateway` string.
fn pearl_gateway(stack: &mod_api::ItemStack) -> &'static Definition {
    let named = stack
        .components()
        .and_then(|c| c.as_nbt().get(&"minecraft:custom_data".into()))
        .and_then(|data| match data {
            Tag::Compound(data) => data.get(&"gateway".into()),
            _ => None,
        })
        .and_then(|id| match id {
            Tag::String(id) => definition(&id.to_string_lossy()),
            _ => None,
        });
    named.unwrap_or_else(|| definition(DEFAULT).expect("the default gateway is built in"))
}

/// The Gate Pearl.
struct Pearl;

#[mod_api::horizon_part]
impl ItemPart for Pearl {
    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return Some(InteractionResult::Success);
        };
        let player = host.player(event.player)?;
        let level = player.level(host)?;
        let dimension = level.dimension().id(host)?;
        let now = level.game_time(host)?;
        let def = pearl_gateway(event.stack);
        let center = [
            f64::from(event.pos[0] + event.face[0]) + 0.5,
            f64::from(event.pos[1] + event.face[1]),
            f64::from(event.pos[2] + event.face[2]) + 0.5,
        ];
        let bar_id = format!("gateways:gateway_{}", gateway_serial());
        let title = Text::translate("gateways.boss_bar.setup", Vec::new());
        let bar = host.create_boss_bar(&bar_id, title).ok().flatten();
        if let Some(bar) = bar {
            bar.add_player(host, player);
        }
        gateways().push(Open {
            run: Gateway::open(def, now),
            dimension,
            center,
            mobs: Vec::new(),
            bar,
            opener: Some(player),
        });
        if let Some(living) = host.living(player.entity()) {
            let left = event.stack.clone().shrink(1);
            let _ = living.set_hand_item(host, event.hand, left);
        }
        Some(InteractionResult::SuccessServer)
    }
}

fn gateway_serial() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Look at the wave's mobs: drop the dead, say whether one strayed.
fn view(host: &mut dyn ServerHost, open: &mut Open) -> WaveView {
    let leash = open.run.def.leash_range;
    let mut out_of_leash = false;
    open.mobs.retain(|&id| {
        let Some(entity) = host.entity(id) else {
            return false;
        };
        let alive = host
            .living(entity)
            .and_then(|l| l.alive(host))
            .unwrap_or(false);
        if alive && let Some(pos) = entity.position(host) {
            let d2: f64 = (0..3).map(|i| (pos[i] - open.center[i]).powi(2)).sum();
            out_of_leash |= d2 > leash * leash;
        }
        alive
    });
    WaveView {
        alive: open.mobs.len(),
        out_of_leash,
    }
}

fn drop_item(host: &mut dyn ServerHost, dimension: &str, at: [f64; 3], id: &str, count: u32) {
    let item: Compound = [
        ("id".into(), string(id)),
        ("count".into(), Tag::Int(count as i32)),
    ]
    .into();
    host.summon(
        dimension,
        "minecraft:item",
        at,
        [("Item".into(), Tag::Compound(item))].into(),
    );
}

fn act(host: &mut dyn ServerHost, open: &mut Open, action: Action) {
    let def = open.run.def;
    match action {
        Action::SpawnWave(i) => {
            let wave = &def.waves[i];
            let n = wave.size();
            let mut k = 0;
            for &(kind, count) in wave.entities {
                for _ in 0..count {
                    let at = spawn_point(open.center, k, n, def.spawn_range);
                    k += 1;
                    let nbt: Compound = [
                        ("Tags".into(), Tag::List(vec![string(MOB_TAG)])),
                        ("PersistenceRequired".into(), Tag::Byte(1)),
                    ]
                    .into();
                    if let Some(id) = host.summon(&open.dimension, kind, at, nbt) {
                        open.mobs.push(id);
                    }
                }
            }
            if let Some(bar) = open.bar {
                let title = Text::translate(
                    "gateways.boss_bar.wave",
                    vec![
                        Text::literal((i + 1).to_string()),
                        Text::literal(def.waves.len().to_string()),
                    ],
                );
                let _ = bar.set_name(host, title);
                bar.set_max(host, n as i32);
            }
        }
        Action::DropRewards(i) => {
            for &(item, count) in def.waves[i].rewards {
                drop_item(host, &open.dimension, open.center, item, count);
            }
        }
        Action::Fail(failure) => {
            for id in std::mem::take(&mut open.mobs) {
                if let Some(entity) = host.entity(id) {
                    entity.discard(host);
                }
            }
            let key = match failure {
                Failure::Timeout => "gateways.fail.timeout",
                Failure::LeftLeash => "gateways.fail.left_leash",
            };
            tell_opener(host, open, key);
        }
        Action::Complete => tell_opener(host, open, "gateways.complete"),
    }
}

fn tell_opener(host: &mut dyn ServerHost, open: &Open, key: &str) {
    if let Some(player) = open.opener {
        let _ = player.tell(host, Text::translate(key, Vec::new()));
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl mod_api::Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let pearl = r.item_key("gateways:gate_pearl");
        r.item_part(pearl, Pearl);
    }

    /// Run the gateways of the level that just ticked.
    fn level_tick_post(&self, host: HostSide, event: &mut LevelTickPostEvent) {
        let HostSide::Server(host) = host else {
            return;
        };
        let mut open = std::mem::take(&mut *gateways());
        if open.is_empty() {
            return;
        }
        let here = event.level.dimension().id(host);
        let now = event.level.game_time(host);
        for gate in open.iter_mut() {
            let (Some(here), Some(now)) = (here.as_deref(), now) else {
                break;
            };
            if gate.dimension != here {
                continue;
            }
            let seen = view(host, gate);
            for action in gate.run.step(now, seen) {
                act(host, gate, action);
            }
            if let Some(bar) = gate.bar {
                bar.set_value(host, gate.mobs.len() as i32);
            }
        }
        open.retain(|gate| {
            let closed = gate.run.phase == Phase::Closed;
            if closed && let Some(bar) = gate.bar {
                bar.remove(host);
            }
            !closed
        });
        // Gateways opened during this tick's actions were pushed meanwhile: keep them too.
        let mut list = gateways();
        open.append(&mut list);
        *list = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiet(alive: usize) -> WaveView {
        WaveView {
            alive,
            out_of_leash: false,
        }
    }

    #[test]
    fn clearing_every_wave_drops_each_reward_and_completes() {
        let def = definition("gateways:basic/slime").unwrap();
        let mut gate = Gateway::open(def, 0);
        assert_eq!(gate.step(59, quiet(0)), vec![]);
        assert_eq!(gate.step(60, quiet(0)), vec![Action::SpawnWave(0)]);
        assert_eq!(gate.step(100, quiet(4)), vec![]);
        assert_eq!(gate.step(200, quiet(0)), vec![Action::DropRewards(0)]);
        // The second wave's setup is 100 ticks.
        assert_eq!(gate.step(299, quiet(0)), vec![]);
        assert_eq!(gate.step(300, quiet(0)), vec![Action::SpawnWave(1)]);
        assert_eq!(
            gate.step(400, quiet(0)),
            vec![Action::DropRewards(1), Action::Complete]
        );
        assert_eq!(gate.phase, Phase::Closed);
        assert_eq!(gate.step(500, quiet(0)), vec![]);
    }

    #[test]
    fn a_wave_that_outlives_its_time_fails_the_gateway() {
        let def = definition("gateways:basic/blaze").unwrap();
        let mut gate = Gateway::open(def, 1000);
        assert_eq!(gate.step(1060, quiet(0)), vec![Action::SpawnWave(0)]);
        assert_eq!(gate.step(1659, quiet(1)), vec![]);
        assert_eq!(
            gate.step(1660, quiet(1)),
            vec![Action::Fail(Failure::Timeout)]
        );
    }

    #[test]
    fn a_mob_past_the_leash_fails_the_gateway() {
        let def = definition("gateways:basic/blaze").unwrap();
        let mut gate = Gateway::open(def, 0);
        gate.step(60, quiet(0));
        let strayed = WaveView {
            alive: 3,
            out_of_leash: true,
        };
        assert_eq!(
            gate.step(61, strayed),
            vec![Action::Fail(Failure::LeftLeash)]
        );
    }

    #[test]
    fn spawn_points_ring_the_gateway() {
        let c = [10.5, 64.0, -3.5];
        let first = spawn_point(c, 0, 4, 3.0);
        let second = spawn_point(c, 1, 4, 3.0);
        assert!((first[0] - 13.5).abs() < 1e-9 && (first[2] + 3.5).abs() < 1e-9);
        assert!((second[0] - 10.5).abs() < 1e-9 && (second[2] + 0.5).abs() < 1e-9);
        assert_eq!(first[1], 64.0);
    }
}
