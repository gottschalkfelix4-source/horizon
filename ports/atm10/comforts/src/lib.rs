// SPDX-License-Identifier: LGPL-3.0-or-later
//! Comforts for Horizon: sleeping bags let you sleep through the night without moving your
//! spawn point; hammocks let you sleep through the day, waking at dusk.
//!
//! Written from Comforts' public description and observed behaviour, never from its code
//! (docs/modpacks.md, "Licences"). What a harness against the NeoForge original still has to
//! confirm is in README.md.
//!
//! How it works: the blocks are beds (bed templates), so vanilla's sleeping runs for them. The
//! hooks change three things: a respawn point at a sleeping bag or hammock is refused
//! (`player_set_spawn`); a hammock may be slept in by day and not by night
//! (`can_player_sleep`, `can_continue_sleeping`); and when the sleepers of a level wake by
//! day, the clock moves on to dusk instead of to the next morning (`sleep_finished_time`).
use std::sync::OnceLock;

use mod_api::{
    BlockKey, CanContinueSleepingEvent, CanPlayerSleepEvent, ClockAdjustment, Hooks, Host,
    LevelRef, PlayerSetSpawnEvent, Registrar, RegistrationContext, ServerHost,
    SleepFinishedTimeEvent, Text,
};

/// The sixteen dye colours, in vanilla's order: one sleeping bag and one hammock each.
pub const COLORS: [&str; 16] = [
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "light_gray",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
];

/// Ticks in a Minecraft day.
pub const DAY: i64 = 24_000;

/// The first and last time of day a player may sleep in a bed when it is not thundering
/// (vanilla's night window).
pub const NIGHT: (i64, i64) = (12_542, 23_459);

/// The time of day a hammock's sleepers wake at.
pub const DUSK: i64 = 13_000;

/// What a comfort block is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    SleepingBag,
    Hammock,
}

/// Whether `time` (a clock time in ticks) falls in the day, when a hammock may be slept in.
pub fn is_day(time: i64) -> bool {
    let t = time.rem_euclid(DAY);
    !(NIGHT.0..=NIGHT.1).contains(&t)
}

/// The next dusk after `time`.
pub fn next_dusk(time: i64) -> i64 {
    let dusk = time - time.rem_euclid(DAY) + DUSK;
    if dusk > time { dusk } else { dusk + DAY }
}

/// The comfort blocks' keys, sleeping bags then hammocks, in [`COLORS`] order.
static KEYS: OnceLock<Vec<(BlockKey, Kind)>> = OnceLock::new();

fn kind_of(block: Option<BlockKey>) -> Option<Kind> {
    let block = block?;
    KEYS.get()?
        .iter()
        .find_map(|(k, kind)| (*k == block).then_some(*kind))
}

fn kind_at(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<Kind> {
    kind_of(level.block(host, pos))
}

/// The level of the online player `uuid`.
fn level_of(host: &dyn Host, uuid: u128) -> Option<LevelRef> {
    host.player(uuid)?.level(host)
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let mut keys = Vec::new();
        for (prefix, kind) in [
            ("sleeping_bag", Kind::SleepingBag),
            ("hammock", Kind::Hammock),
        ] {
            for c in COLORS {
                keys.push((r.block_key(&format!("comforts:{prefix}_{c}")), kind));
            }
        }
        let _ = KEYS.set(keys);
    }

    /// Sleeping in a sleeping bag or a hammock keeps the old respawn point.
    fn player_set_spawn(
        &self,
        host: &mut dyn ServerHost,
        event: &mut PlayerSetSpawnEvent<'_>,
    ) -> mod_api::Flow {
        let (Some(pos), false) = (event.pos, event.forced) else {
            return mod_api::Flow::Continue;
        };
        let read: &dyn Host = &*host;
        let Some(level) = level_of(read, event.player) else {
            return mod_api::Flow::Continue;
        };
        if level.dimension_id() == event.dimension && kind_at(read, level, pos).is_some() {
            event.cancelled = true;
        }
        mod_api::Flow::Continue
    }

    /// A hammock by day lets its player sleep whatever vanilla's night rule says; by night it
    /// refuses.
    fn can_player_sleep(&self, host: &mut dyn ServerHost, event: &mut CanPlayerSleepEvent<'_>) {
        let read: &dyn Host = &*host;
        let Some(level) = level_of(read, event.player) else {
            return;
        };
        if kind_at(read, level, event.pos) != Some(Kind::Hammock) {
            return;
        }
        let Some(time) = level.day_time(read) else {
            return;
        };
        if is_day(time) {
            if event.problem.as_deref() == Some("not_possible_now") {
                *event.problem = None;
            }
        } else if event.problem.is_none() {
            *event.problem = Some("other_problem".into());
            if let Some(player) = host.player(event.player) {
                let _ = player.action_bar(host, Text::translate("comforts.hammock.night", vec![]));
            }
        }
    }

    /// A hammock's sleeper sleeps on by day and wakes at night.
    fn can_continue_sleeping(
        &self,
        host: &mut dyn ServerHost,
        event: &mut CanContinueSleepingEvent<'_>,
    ) -> Option<bool> {
        let read: &dyn Host = &*host;
        let level = level_of(read, event.entity)?;
        if kind_at(read, level, event.pos)? != Kind::Hammock {
            return None;
        }
        Some(is_day(level.day_time(read)?))
    }

    /// Sleeping through the day in hammocks ends at dusk. A day sleep in beds (a
    /// thunderstorm's) keeps vanilla's morning.
    fn sleep_finished_time(
        &self,
        host: &mut dyn ServerHost,
        event: &mut SleepFinishedTimeEvent<'_>,
    ) -> mod_api::Flow {
        if !is_day(event.current) {
            return mod_api::Flow::Continue;
        }
        let levels = host.levels().list_levels();
        let read: &dyn Host = &*host;
        let Some(level) = levels
            .into_iter()
            .find(|l| l.dimension_id() == event.dimension)
        else {
            return mod_api::Flow::Continue;
        };
        let in_hammock = level
            .online_players(read)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|p| read.living(p.entity())?.sleeping_pos(read))
            .any(|pos| kind_at(read, level, pos) == Some(Kind::Hammock));
        if in_hammock {
            *event.adjustment = ClockAdjustment::Absolute(next_dusk(event.current));
        }
        mod_api::Flow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hammock_hours_are_the_complement_of_the_bed_night() {
        assert!(is_day(1_000));
        assert!(is_day(12_541));
        assert!(!is_day(12_542));
        assert!(!is_day(18_000));
        assert!(!is_day(23_459));
        assert!(is_day(23_460));
        // A later day, and a clock run backwards past zero.
        assert!(is_day(5 * 24_000 + 6_000));
        assert!(!is_day(-6_000));
    }

    #[test]
    fn wakes_at_the_coming_dusk() {
        assert_eq!(next_dusk(1_000), 13_000);
        assert_eq!(next_dusk(13_000), 37_000);
        assert_eq!(next_dusk(23_500), 37_000);
        assert_eq!(next_dusk(48_000 + 2_000), 61_000);
    }
}
