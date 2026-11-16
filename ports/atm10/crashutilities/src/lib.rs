//! Crash Utilities for Horizon: server admin commands under `/cu` for finding what loads a
//! server down (entities, chunks, memory) and for helping stuck players, plus an optional clear
//! of ground items once there are too many.
//!
//! Written from Crash Utilities' public description and observed behaviour, never from its code
//! (docs/modpacks.md, "Licences"). What is ported, what is assumed and what is left is in
//! README.md.
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use mod_api::{
    Command, CommandArgumentValue, ConfigPath, EntityRef, Flow, Hooks, LevelRef, PlayerRef,
    RegisterCommandsEvent, RunModCommandEvent, ServerHost, ServerStartedEvent, ServerTickEvent,
    ServerTickPhase, Text,
};

pub mod config;
pub mod stats;
mod text;

use config::Config;
use text::{lit, tr};

/// The operator level every `/cu` command needs.
const LEVEL: u8 = 2;
/// How many rows `entities find` prints.
const FIND_ROWS: usize = 10;

static CONFIG: Mutex<Option<Config>> = Mutex::new(None);
static TICKS: AtomicU64 = AtomicU64::new(0);

fn feedback(host: &mut dyn ServerHost, line: Text) {
    let _ = host.commands().feedback(line);
}

fn dimension_name(host: &dyn ServerHost, level: LevelRef) -> String {
    level.dimension().id(host).unwrap_or_else(|| "?".to_owned())
}

/// The levels a command looks at: the one named, or all.
fn levels(host: &dyn ServerHost, named: Option<&CommandArgumentValue>) -> Vec<LevelRef> {
    match named {
        Some(CommandArgumentValue::Dimension(level)) => vec![*level],
        _ => host.levels().list_levels(),
    }
}

fn entity_type(host: &dyn ServerHost, entity: EntityRef) -> Option<String> {
    entity.kind(host).map(|key| key.id(host))
}

fn wanted_kind(event: &RunModCommandEvent<'_>) -> Option<String> {
    match event.command.argument("kind") {
        Some(CommandArgumentValue::Id(id)) => Some(id.to_string()),
        _ => None,
    }
}

fn player_arg(event: &RunModCommandEvent<'_>) -> Option<PlayerRef> {
    match event.command.argument("player") {
        Some(CommandArgumentValue::Player(player)) => Some(*player),
        _ => None,
    }
}

fn player_name(host: &dyn ServerHost, player: PlayerRef) -> String {
    player.name(host).unwrap_or_default()
}

/// `/cu entities list [dimension]`: every entity type and its count, per level.
fn entities_list(host: &mut dyn ServerHost, event: &RunModCommandEvent<'_>) {
    for level in levels(host, event.command.argument("dimension")) {
        let entities = level.all_entities(host).unwrap_or_default();
        let kinds: Vec<String> = entities
            .iter()
            .filter_map(|e| entity_type(host, *e))
            .collect();
        let name = dimension_name(host, level);
        feedback(
            host,
            tr(
                "crashutilities.entities.header",
                vec![lit(name), lit(kinds.len())],
            ),
        );
        for (kind, count) in stats::ranked(kinds) {
            feedback(
                host,
                tr("crashutilities.entities.line", vec![lit(kind), lit(count)]),
            );
        }
    }
}

/// `/cu entities find <kind> [dimension]`: the chunks holding the most of one entity type.
fn entities_find(host: &mut dyn ServerHost, event: &RunModCommandEvent<'_>) {
    let Some(kind) = wanted_kind(event) else {
        return;
    };
    let mut chunks = Vec::new();
    for level in levels(host, event.command.argument("dimension")) {
        let name = dimension_name(host, level);
        for entity in level.all_entities(host).unwrap_or_default() {
            if entity_type(host, entity).as_deref() != Some(kind.as_str()) {
                continue;
            }
            if let Some(pos) = entity.position(host) {
                chunks.push((name.clone(), stats::chunk_of(pos)));
            }
        }
    }
    if chunks.is_empty() {
        feedback(host, tr("crashutilities.entities.none", vec![]));
        return;
    }
    feedback(
        host,
        tr("crashutilities.entities.find.header", vec![lit(&kind)]),
    );
    for ((dimension, [x, z]), count) in stats::ranked(chunks).into_iter().take(FIND_ROWS) {
        // Clicking the line teleports the operator to the chunk's middle.
        let tp = format!(
            "/execute in {dimension} run tp @s {} ~ {}",
            x * 16 + 8,
            z * 16 + 8
        );
        let mut line = tr(
            "crashutilities.entities.find.line",
            vec![lit(x), lit(z), lit(&dimension), lit(count)],
        );
        line.style.click = Some(mod_api::text::TextClick::SuggestCommand(tp));
        feedback(host, line);
    }
}

/// `/cu entities remove <kind> [dimension]`: discard every loaded entity of one type.
fn entities_remove(host: &mut dyn ServerHost, event: &RunModCommandEvent<'_>) {
    let Some(kind) = wanted_kind(event) else {
        return;
    };
    let mut removed = 0;
    for level in levels(host, event.command.argument("dimension")) {
        for entity in level.all_entities(host).unwrap_or_default() {
            if entity_type(host, entity).as_deref() == Some(kind.as_str()) && entity.discard(host) {
                removed += 1;
            }
        }
    }
    feedback(
        host,
        tr(
            "crashutilities.entities.removed",
            vec![lit(removed), lit(kind)],
        ),
    );
}

/// `/cu loadedchunks [dimension]`: how many chunks each level holds loaded.
fn loaded_chunks(host: &mut dyn ServerHost, event: &RunModCommandEvent<'_>) {
    for level in levels(host, event.command.argument("dimension")) {
        let count = level.loaded_chunks(host).map_or(0, |c| c.len());
        let name = dimension_name(host, level);
        feedback(
            host,
            tr("crashutilities.chunks.line", vec![lit(name), lit(count)]),
        );
    }
}

/// `/cu unstuck <player>`: send a player to the world spawn in the Overworld.
fn unstuck(host: &mut dyn ServerHost, event: &RunModCommandEvent<'_>) {
    let Some(player) = player_arg(event) else {
        return;
    };
    let name = player_name(host, player);
    let spawn = host
        .level("minecraft:overworld")
        .and_then(|level| level.respawn_data(host));
    let moved = spawn.is_some_and(|spawn| {
        let pos = [
            spawn.pos[0] as f64 + 0.5,
            spawn.pos[1] as f64,
            spawn.pos[2] as f64 + 0.5,
        ];
        player.entity().teleport_to_level(
            host,
            "minecraft:overworld",
            pos,
            Some([spawn.yaw, spawn.pitch]),
        )
    });
    let key = if moved {
        "crashutilities.unstuck.done"
    } else {
        "crashutilities.unstuck.failed"
    };
    feedback(host, tr(key, vec![lit(name)]));
}

/// `/cu teleport <player> <dimension> <pos>`: move a player into another level.
fn teleport(host: &mut dyn ServerHost, event: &RunModCommandEvent<'_>) {
    let (
        Some(player),
        Some(CommandArgumentValue::Dimension(level)),
        Some(CommandArgumentValue::Vec3(pos)),
    ) = (
        player_arg(event),
        event.command.argument("dimension"),
        event.command.argument("pos"),
    )
    else {
        return;
    };
    let name = player_name(host, player);
    let dimension = dimension_name(host, *level);
    if player
        .entity()
        .teleport_to_level(host, &dimension, *pos, None)
    {
        feedback(
            host,
            tr(
                "crashutilities.teleport.done",
                vec![lit(name), lit(dimension)],
            ),
        );
    } else {
        feedback(host, tr("crashutilities.unstuck.failed", vec![lit(name)]));
    }
}

/// `/cu inventory read <player>`: every filled slot of an online player.
fn inventory_read(host: &mut dyn ServerHost, event: &RunModCommandEvent<'_>) {
    let Some(player) = player_arg(event) else {
        return;
    };
    let name = player_name(host, player);
    let slots = player.inventory_slots(host).unwrap_or_default();
    if slots.is_empty() {
        feedback(host, tr("crashutilities.inventory.empty", vec![lit(name)]));
        return;
    }
    feedback(host, tr("crashutilities.inventory.header", vec![lit(name)]));
    for (slot, stack) in slots {
        let item = stack.item().id(host);
        feedback(
            host,
            tr(
                "crashutilities.inventory.line",
                vec![lit(slot), lit(stack.count()), lit(item)],
            ),
        );
    }
}

/// `/cu inventory remove <player> <slot>`: empty one slot of an online player.
fn inventory_remove(host: &mut dyn ServerHost, event: &RunModCommandEvent<'_>) {
    let (Some(player), Some(CommandArgumentValue::Integer(slot))) =
        (player_arg(event), event.command.argument("slot"))
    else {
        return;
    };
    let name = player_name(host, player);
    let done = u32::try_from(*slot)
        .ok()
        .is_some_and(|slot| player.set_inventory_slot(host, slot, None).is_ok());
    let key = if done {
        "crashutilities.inventory.removed"
    } else {
        "crashutilities.inventory.failed"
    };
    feedback(host, tr(key, vec![lit(slot), lit(name)]));
}

/// `/cu memory`: the server process's resident memory.
fn memory(host: &mut dyn ServerHost) {
    let used = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| stats::rss_megabytes(&status));
    let line = match used {
        Some(mb) => tr("crashutilities.memory.used", vec![lit(mb)]),
        None => tr("crashutilities.memory.unavailable", vec![]),
    };
    feedback(host, line);
}

/// The ground-item clear, checked at the end of a server tick.
fn item_clear(host: &mut dyn ServerHost) {
    let tick = TICKS.fetch_add(1, Ordering::Relaxed) + 1;
    let Some(config) = CONFIG.lock().ok().and_then(|c| c.clone()) else {
        return;
    };
    // Scan the levels only on a tick the clear may run.
    if !stats::should_clear(&config.item_clear, tick, usize::MAX) {
        return;
    }
    let mut items = Vec::new();
    for level in host.levels().list_levels() {
        for entity in level.all_entities(host).unwrap_or_default() {
            if entity_type(host, entity).as_deref() == Some("minecraft:item") {
                items.push(entity);
            }
        }
    }
    if !stats::should_clear(&config.item_clear, tick, items.len()) {
        return;
    }
    let removed = items.into_iter().filter(|e| e.discard(host)).count();
    let _ = host
        .messages()
        .broadcast(tr("crashutilities.itemclear.cleared", vec![lit(removed)]));
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn server_started(&self, host: &mut dyn ServerHost, _: &mut ServerStartedEvent) {
        let config = ConfigPath::new("crashutilities.json")
            .ok()
            .and_then(|path| host.config_files().read(&path).ok())
            .map(|bytes| Config::parse(&bytes))
            .unwrap_or_default();
        if let Ok(mut slot) = CONFIG.lock() {
            *slot = Some(config);
        }
        TICKS.store(0, Ordering::Relaxed);
    }

    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        if matches!(
            event.phase,
            ServerTickPhase::ServerTickEnd {
                world_running: true,
                ..
            }
        ) {
            item_clear(host);
        }
    }

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.extend(
            [
                "cu entities list [dimension:dimension]",
                "cu entities find <kind:resource:minecraft:entity_type> [dimension:dimension]",
                "cu entities remove <kind:resource:minecraft:entity_type> [dimension:dimension]",
                "cu loadedchunks [dimension:dimension]",
                "cu unstuck <player:player>",
                "cu teleport <player:player> <dimension:dimension> <pos:vec3>",
                "cu inventory read <player:player>",
                "cu inventory remove <player:player> <slot:integer>",
                "cu memory",
            ]
            .map(|usage| Command::op(usage, LEVEL)),
        );
    }

    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        let path: Vec<&str> = event.command.literals.iter().map(String::as_str).collect();
        match path.as_slice() {
            ["cu", "entities", "list"] => entities_list(host, event),
            ["cu", "entities", "find"] => entities_find(host, event),
            ["cu", "entities", "remove"] => entities_remove(host, event),
            ["cu", "loadedchunks"] => loaded_chunks(host, event),
            ["cu", "unstuck"] => unstuck(host, event),
            ["cu", "teleport"] => teleport(host, event),
            ["cu", "inventory", "read"] => inventory_read(host, event),
            ["cu", "inventory", "remove"] => inventory_remove(host, event),
            ["cu", "memory"] => memory(host),
            _ => return Flow::Continue,
        }
        Flow::Cancel
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::ItemClear;

    #[test]
    fn counts_are_ranked_largest_first_with_ties_by_name() {
        let rows = stats::ranked(["zombie", "cow", "zombie", "bat", "cow", "zombie"]);
        assert_eq!(rows, vec![("zombie", 3), ("cow", 2), ("bat", 1)]);
    }

    #[test]
    fn negative_positions_fall_in_negative_chunks() {
        assert_eq!(stats::chunk_of([-0.5, 64.0, 15.9]), [-1, 0]);
        assert_eq!(stats::chunk_of([-16.0, 0.0, -17.0]), [-1, -2]);
    }

    #[test]
    fn resident_memory_is_read_from_proc_status() {
        let status = "Name:\tjava\nVmPeak:\t 9000000 kB\nVmRSS:\t 2097152 kB\nThreads:\t40\n";
        assert_eq!(stats::rss_megabytes(status), Some(2048));
        assert_eq!(stats::rss_megabytes("Name:\tx\n"), None);
    }

    #[test]
    fn the_item_clear_runs_only_on_its_interval_and_over_the_limit() {
        let config = ItemClear {
            enabled: true,
            maximum: 100,
            interval_ticks: 20,
        };
        assert!(stats::should_clear(&config, 40, 101));
        assert!(!stats::should_clear(&config, 40, 100));
        assert!(!stats::should_clear(&config, 41, 500));
        let off = ItemClear {
            enabled: false,
            ..config
        };
        assert!(!stats::should_clear(&off, 40, 500));
    }

    #[test]
    fn a_partial_config_keeps_the_other_defaults() {
        let config = Config::parse(br#"{"item_clear": {"enabled": true}}"#);
        assert!(config.item_clear.enabled);
        assert_eq!(config.item_clear.maximum, ItemClear::default().maximum);
        assert_eq!(Config::parse(b"not json"), Config::default());
    }

    #[test]
    fn feedback_carries_its_english_fallback() {
        let line = tr("crashutilities.memory.used", vec![lit(12)]);
        match line.content {
            mod_api::text::TextContent::Translatable { key, fallback, .. } => {
                assert_eq!(key, "crashutilities.memory.used");
                assert_eq!(
                    fallback.as_deref(),
                    Some("Memory in use by the server process: %s MB")
                );
            }
            other => panic!("{other:?}"),
        }
    }
}
