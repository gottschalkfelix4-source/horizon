//! I'm Fast for Horizon: players may move faster than vanilla's server allows without being
//! pulled back ("moved too quickly").
//!
//! Written from I'm Fast's public description, never from its code (docs/modpacks.md,
//! "Licences"). The original patches the checks out with a mixin; the mod API has no hook for
//! them, so this port turns them off through vanilla's own game rules
//! (`player_movement_check`, `elytra_movement_check`) while the server runs, and puts back what
//! the world had when it stops, so removing the mod leaves the world as it was. See README.md.
use mod_api::{
    ConfigPath, GameRuleValue, Hooks, LevelRef, ServerHost, ServerStartedEvent, ServerStoppingEvent,
};
use serde::{Deserialize, Serialize};

/// The vanilla rules this mod turns off.
pub const RULES: [&str; 2] = [
    "minecraft:player_movement_check",
    "minecraft:elytra_movement_check",
];

/// What the world had before the mod changed it, kept in the world's
/// `horizon-mod-data/imfast/saved_rules.json` until the server stops (so a crash still leaves a
/// record to restore from at the next start).
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Saved {
    /// `(rule, value)` pairs.
    pub rules: Vec<(String, bool)>,
}

impl Saved {
    /// Merge the values read now with a record left by an earlier run: the earlier record wins,
    /// since the values read now may be this mod's own.
    pub fn merged(previous: Option<Saved>, now: Vec<(String, bool)>) -> Saved {
        match previous {
            Some(previous) if !previous.rules.is_empty() => previous,
            _ => Saved { rules: now },
        }
    }
}

fn saved_path() -> Option<ConfigPath> {
    ConfigPath::new("saved_rules.json").ok()
}

fn overworld(host: &dyn ServerHost) -> Option<LevelRef> {
    host.level("minecraft:overworld")
}

fn read_saved(host: &dyn ServerHost) -> Option<Saved> {
    let bytes = host.world_data_files().read(&saved_path()?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_saved(host: &dyn ServerHost, saved: &Saved) {
    if let (Some(path), Ok(bytes)) = (saved_path(), serde_json::to_vec(saved)) {
        let _ = host.world_data_files().write(&path, &bytes);
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn server_started(&self, host: &mut dyn ServerHost, _: &mut ServerStartedEvent) {
        let Some(level) = overworld(host) else {
            return;
        };
        let now = RULES
            .iter()
            .filter_map(|rule| match level.game_rule(host, rule) {
                Ok(Some(GameRuleValue::Bool(value))) => Some(((*rule).to_owned(), value)),
                _ => None,
            })
            .collect();
        let saved = Saved::merged(read_saved(host), now);
        write_saved(host, &saved);
        for rule in RULES {
            level.set_game_rule(host, rule, GameRuleValue::Bool(false));
        }
    }

    fn server_stopping(&self, host: &mut dyn ServerHost, _: &mut ServerStoppingEvent) {
        let (Some(level), Some(saved)) = (overworld(host), read_saved(host)) else {
            return;
        };
        for (rule, value) in &saved.rules {
            level.set_game_rule(host, rule, GameRuleValue::Bool(*value));
        }
        write_saved(host, &Saved::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_left_by_an_earlier_run_is_kept() {
        let earlier = Saved {
            rules: vec![("minecraft:player_movement_check".into(), true)],
        };
        let now = vec![("minecraft:player_movement_check".into(), false)];
        assert_eq!(Saved::merged(Some(earlier.clone()), now), earlier);
    }

    #[test]
    fn without_a_record_the_current_values_are_saved() {
        let now = vec![("minecraft:elytra_movement_check".into(), true)];
        assert_eq!(
            Saved::merged(Some(Saved::default()), now.clone()).rules,
            now
        );
        assert_eq!(Saved::merged(None, now.clone()).rules, now);
    }
}
