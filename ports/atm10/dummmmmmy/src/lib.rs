//! MmmMmmMmmMmm for Horizon, first slice: the target dummy that takes every hit, never dies, and
//! tells its attacker the damage of each hit and their damage per second.
//!
//! Written from the mod's public description and from how the dummy plays, never from its code
//! (docs/modpacks.md, "Licences"). What is from memory, and the workarounds, are in README.md.
//!
//! How it works:
//! - `dummmmmmy:target_dummy` used on a block stands a dummy on the face clicked. Horizon has no
//!   way yet for a mod to declare a living entity with its own class (README, "Needs from
//!   Horizon"), so the dummy is vanilla's `minecraft:mannequin`, which has no AI and does not move
//!   when hit, tagged `dummmmmmy.target_dummy`.
//! - `living_damage_pre` (after armour, as the original measures) records each hit on a dummy
//!   and sets the health damage to 0, so it never dies. `/kill` and the void still remove it.
//! - After each level tick the recorded hits are reported: the hit's damage over the attacker's
//!   hotbar, and once they have stopped hitting for [`COMBAT_END_TICKS`], their damage per second
//!   over the whole fight in chat.
//! - Sneaking and using an empty hand on a dummy takes it down and drops its item.
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use mod_api::{
    Compound, EntityInteractEvent, EntityJoinLevelEvent, EntityLeaveLevelEvent, Flow, Hand,
    HostSide, InteractionResult, ItemKey, ItemPart, LevelRandomHost, LevelTickPostEvent,
    LivingDamagePreEvent, Registrar, RegistrationContext, ServerHost, Tag, Text, UseOnEvent,
};

/// The tag that makes a mannequin a target dummy.
pub const DUMMY_TAG: &str = "dummmmmmy.target_dummy";
const ITEM: &str = "dummmmmmy:target_dummy";
const STAND_IN: &str = "minecraft:mannequin";
/// Ticks without a hit after which a fight is over and its DPS is told (from memory: a few
/// seconds).
pub const COMBAT_END_TICKS: i64 = 60;
/// The shortest span a fight's DPS is measured over: one second, so a single hit reads as its
/// own damage per second rather than twenty times it.
pub const MIN_SPAN_TICKS: i64 = 20;

/// Whether a hit of `damage_type` on a dummy is absorbed (recorded and left without health
/// damage). `/kill` and falling out of the world still take it, so a dummy can always be removed.
pub fn absorbs(damage_type: &str) -> bool {
    !matches!(
        damage_type,
        "minecraft:generic_kill" | "minecraft:out_of_world"
    )
}

/// One attacker's fight with one dummy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fight {
    pub first_tick: i64,
    pub last_tick: i64,
    pub total: f32,
}

impl Fight {
    pub fn start(tick: i64, damage: f32) -> Self {
        Self {
            first_tick: tick,
            last_tick: tick,
            total: damage,
        }
    }
    pub fn hit(&mut self, tick: i64, damage: f32) {
        self.last_tick = tick;
        self.total += damage;
    }
    /// Whether the fight has ended at `now`.
    pub fn over(&self, now: i64) -> bool {
        now - self.last_tick >= COMBAT_END_TICKS
    }
    /// Damage per second from the first hit to the last, over at least [`MIN_SPAN_TICKS`].
    pub fn dps(&self) -> f32 {
        let span = (self.last_tick - self.first_tick).max(MIN_SPAN_TICKS);
        self.total * 20.0 / span as f32
    }
}

/// A number as the dummy shows it: one decimal, without a trailing `.0`.
pub fn format_amount(amount: f32) -> String {
    let rounded = (amount * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.1}")
    }
}

/// A hit recorded during a level's tick, reported after it.
struct Hit {
    attacker: i32,
    damage: f32,
}

#[derive(Default)]
struct State {
    /// The ids of the dummies standing in loaded levels.
    dummies: HashSet<u32>,
    /// Hits not yet reported.
    pending: Vec<Hit>,
    /// Running fights, by attacker id.
    fights: HashMap<i32, Fight>,
}

fn state() -> std::sync::MutexGuard<'static, State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

static KEY: OnceLock<ItemKey> = OnceLock::new();

fn string(value: &str) -> Tag {
    Tag::String(value.into())
}

/// The dummy's item.
struct DummyItem;

#[mod_api::horizon_part]
impl ItemPart for DummyItem {
    /// Stand a dummy on the face clicked, facing the player.
    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return Some(InteractionResult::Success);
        };
        let player = host.player(event.player)?;
        let dimension = player.level(host)?.dimension().id(host)?;
        let at = [
            f64::from(event.pos[0] + event.face[0]) + 0.5,
            f64::from(event.pos[1] + event.face[1]),
            f64::from(event.pos[2] + event.face[2]) + 0.5,
        ];
        let yaw = player
            .entity()
            .rotation(host)
            .map_or(0.0, |[yaw, _]| yaw + 180.0);
        let nbt: Compound = [
            ("Tags".into(), Tag::List(vec![string(DUMMY_TAG)])),
            ("immovable".into(), Tag::Byte(1)),
            (
                "Rotation".into(),
                Tag::List(vec![Tag::Float(yaw.to_bits()), Tag::Float(0)]),
            ),
        ]
        .into();
        let id = host.summon(&dimension, STAND_IN, at, nbt)?;
        state().dummies.insert(id);
        if let Some(living) = host.living(player.entity()) {
            let left = event.stack.clone().shrink(1);
            let _ = living.set_hand_item(host, event.hand, left);
        }
        Some(InteractionResult::SuccessServer)
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl mod_api::Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let key = r.item_key(ITEM);
        let _ = KEY.set(key);
        r.item_part(key, DummyItem);
    }

    /// Remember a dummy loaded from its chunk.
    fn entity_join_level(
        &self,
        host: &mut dyn ServerHost,
        event: &mut EntityJoinLevelEvent<'_>,
    ) -> Flow {
        if event.kind == STAND_IN
            && let Some(entity) = host.entity(event.entity)
            && entity
                .tags(host)
                .is_some_and(|tags| tags.iter().any(|t| t == DUMMY_TAG))
        {
            state().dummies.insert(event.entity);
        }
        Flow::Continue
    }

    fn entity_leave_level(&self, _: &mut dyn ServerHost, event: &mut EntityLeaveLevelEvent<'_>) {
        if event.kind == STAND_IN {
            state().dummies.remove(&event.entity);
        }
    }

    /// Record a hit on a dummy and keep its health.
    fn living_damage_pre(&self, _: &mut dyn LevelRandomHost, event: &mut LivingDamagePreEvent<'_>) {
        if event.kind != STAND_IN || !absorbs(event.source.damage_type) {
            return;
        }
        let Ok(id) = u32::try_from(event.entity) else {
            return;
        };
        let mut state = state();
        if !state.dummies.contains(&id) {
            return;
        }
        state.pending.push(Hit {
            attacker: event.source.causing,
            damage: event.container.new_damage(),
        });
        event.container.set_new_damage(0.0);
    }

    /// Report this tick's hits and the fights that ended.
    fn level_tick_post(&self, host: HostSide, event: &mut LevelTickPostEvent) {
        let HostSide::Server(host) = host else {
            return;
        };
        let Some(now) = event.level.game_time(host) else {
            return;
        };
        let (hits, ended) = {
            let mut state = state();
            if state.pending.is_empty() && state.fights.is_empty() {
                return;
            }
            let hits = std::mem::take(&mut state.pending);
            for hit in &hits {
                state
                    .fights
                    .entry(hit.attacker)
                    .and_modify(|fight| fight.hit(now, hit.damage))
                    .or_insert_with(|| Fight::start(now, hit.damage));
            }
            let ended: Vec<(i32, Fight)> = state
                .fights
                .iter()
                .filter(|(_, fight)| fight.over(now))
                .map(|(attacker, fight)| (*attacker, *fight))
                .collect();
            for (attacker, _) in &ended {
                state.fights.remove(attacker);
            }
            (hits, ended)
        };
        for hit in hits {
            if let Some(player) = attacking_player(host, hit.attacker) {
                let line = Text::translate(
                    "message.dummmmmmy.damage",
                    vec![Text::literal(format_amount(hit.damage))],
                );
                let _ = player.action_bar(host, line);
            }
        }
        for (attacker, fight) in ended {
            if let Some(player) = attacking_player(host, attacker) {
                let line = Text::translate(
                    "message.dummmmmmy.dps",
                    vec![
                        Text::literal(format_amount(fight.dps())),
                        Text::literal(format_amount(fight.total)),
                    ],
                );
                let _ = player.tell(host, line);
            }
        }
    }

    /// Sneak and use an empty hand on a dummy to take it down.
    fn entity_interact(
        &self,
        host: HostSide,
        event: &mut EntityInteractEvent<'_>,
    ) -> Option<InteractionResult> {
        let id = u32::try_from(event.entity).ok()?;
        if event.kind != STAND_IN || event.hand != Hand::Main || event.stack.is_some() {
            return None;
        }
        let HostSide::Server(host) = host else {
            return None;
        };
        if !state().dummies.contains(&id) {
            return None;
        }
        let player = host.players().acting_player()?;
        if player.entity().is_shift_key_down(host) != Some(true) {
            return None;
        }
        let dummy = host.entity(id)?;
        let at = dummy.position(host)?;
        let dimension = dummy.level(host)?.dimension().id(host)?;
        let item: Compound = [("id".into(), string(ITEM)), ("count".into(), Tag::Int(1))].into();
        host.summon(
            &dimension,
            "minecraft:item",
            at,
            [("Item".into(), Tag::Compound(item))].into(),
        );
        dummy.discard(host);
        state().dummies.remove(&id);
        Some(InteractionResult::SuccessServer)
    }
}

/// The player behind the entity id `attacker`, if it is one.
fn attacking_player(host: &mut dyn ServerHost, attacker: i32) -> Option<mod_api::PlayerRef> {
    let entity = host.entity(u32::try_from(attacker).ok()?)?;
    let uuid = entity.uuid(host)?;
    host.player(uuid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dps_is_the_fights_damage_over_its_span() {
        // 4 hits of 7 over 3 seconds (60 ticks): 28 / 3.
        let mut fight = Fight::start(100, 7.0);
        for tick in [120, 140, 160] {
            fight.hit(tick, 7.0);
        }
        assert!((fight.dps() - 28.0 / 3.0).abs() < 1e-5);
    }

    #[test]
    fn a_single_hit_reads_as_its_damage_per_second() {
        assert_eq!(Fight::start(5, 9.0).dps(), 9.0);
    }

    #[test]
    fn a_fight_ends_after_three_quiet_seconds() {
        let fight = Fight::start(1000, 1.0);
        assert!(!fight.over(1059));
        assert!(fight.over(1060));
    }

    #[test]
    fn kill_and_the_void_still_remove_a_dummy() {
        assert!(absorbs("minecraft:player_attack"));
        assert!(absorbs("minecraft:on_fire"));
        assert!(!absorbs("minecraft:generic_kill"));
        assert!(!absorbs("minecraft:out_of_world"));
    }

    #[test]
    fn amounts_show_one_decimal_without_a_trailing_zero() {
        assert_eq!(format_amount(7.0), "7");
        assert_eq!(format_amount(4.25), "4.3");
        assert_eq!(format_amount(0.04), "0");
    }
}
