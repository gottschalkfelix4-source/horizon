//! WITS (What Is This Structure) for Horizon: `/wits` names the structures at the player's
//! position, `/witsop [dimension] [pos]` lets an operator check anywhere.
//!
//! Written from WITS' public description and observed behaviour, never from its code
//! (docs/modpacks.md, "Licences"). See README.md for what is assumed.
//!
//! How: Horizon's mod API has no "structures at this position" read yet. Vanilla's
//! `location_check` predicate has one (`LocationPredicate.structures`: inside one of the
//! structure's pieces), so this mod ships one predicate per structure
//! (`data/wits/predicate/<namespace>/<structure>.json`) and asks the server's command runner
//! `execute in <dimension> positioned <pos> if predicate wits:<namespace>/<structure>` for each.
//! Only structures that have a predicate file are found: vanilla's, for now.
use mod_api::{
    Command, CommandArgumentValue, Flow, Hooks, RegisterCommandsEvent, RunModCommandEvent,
    ServerHost, Text,
};

mod text;
use text::{lit, tr};

/// The structures this mod has predicates for, in `data/wits/predicate/minecraft/`.
pub const VANILLA_STRUCTURES: &[&str] = &[
    "ancient_city",
    "bastion_remnant",
    "buried_treasure",
    "desert_pyramid",
    "end_city",
    "fortress",
    "igloo",
    "jungle_pyramid",
    "mansion",
    "mineshaft",
    "mineshaft_mesa",
    "monument",
    "nether_fossil",
    "ocean_ruin_cold",
    "ocean_ruin_warm",
    "pillager_outpost",
    "ruined_portal",
    "ruined_portal_desert",
    "ruined_portal_jungle",
    "ruined_portal_mountain",
    "ruined_portal_nether",
    "ruined_portal_ocean",
    "ruined_portal_swamp",
    "shipwreck",
    "shipwreck_beached",
    "stronghold",
    "swamp_hut",
    "trail_ruins",
    "trial_chambers",
    "village_desert",
    "village_plains",
    "village_savanna",
    "village_snowy",
    "village_taiga",
];

/// The predicate that tests for `structure` (`minecraft:village_plains`).
pub fn predicate_id(structure: &str) -> String {
    let (namespace, path) = structure
        .split_once(':')
        .unwrap_or(("minecraft", structure));
    format!("wits:{namespace}/{path}")
}

/// The console line that succeeds when `pos` in `dimension` is inside `structure`.
pub fn probe_line(dimension: &str, pos: [i32; 3], structure: &str) -> String {
    format!(
        "execute in {dimension} positioned {} {} {} if predicate {}",
        pos[0],
        pos[1],
        pos[2],
        predicate_id(structure)
    )
}

/// The block a command's execution position stands in.
pub fn block_of(position: [f64; 3]) -> [i32; 3] {
    position.map(|c| c.floor() as i32)
}

/// The structures at `pos`, sorted. The registry is read so a structure the world does not have
/// (a data pack removed it) is not probed.
fn structures_at(host: &mut dyn ServerHost, dimension: &str, pos: [i32; 3]) -> Vec<String> {
    let known: Option<Vec<String>> = host
        .registry_entries("minecraft:worldgen/structure")
        .map(|ids| ids.iter().map(ToString::to_string).collect());
    let mut found = Vec::new();
    for path in VANILLA_STRUCTURES {
        let id = format!("minecraft:{path}");
        if known.as_ref().is_some_and(|k| !k.contains(&id)) {
            continue;
        }
        if host
            .commands()
            .run_command(&probe_line(dimension, pos, &id))
        {
            found.push(id);
        }
    }
    found.sort();
    found
}

fn report(host: &mut dyn ServerHost, dimension: &str, pos: [i32; 3]) {
    let found = structures_at(host, dimension, pos);
    let place = vec![lit(pos[0]), lit(pos[1]), lit(pos[2]), lit(dimension)];
    let lines: Vec<Text> = if found.is_empty() {
        vec![tr("wits.none", place)]
    } else {
        std::iter::once(tr("wits.header", place))
            .chain(found.into_iter().map(|id| tr("wits.line", vec![lit(id)])))
            .collect()
    };
    for line in lines {
        let _ = host.commands().feedback(line);
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.push(Command::new("wits"));
        event.commands.push(Command::op(
            "witsop [dimension:dimension] [pos:block_pos]",
            2,
        ));
    }

    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        let command = event.command;
        match command.literals.first().map(String::as_str) {
            Some("wits" | "witsop") => {}
            _ => return Flow::Continue,
        }
        let dimension = match command.argument("dimension") {
            Some(CommandArgumentValue::Dimension(level)) => level.dimension().id(host),
            _ => command.dimension.id(host),
        }
        .unwrap_or_else(|| "minecraft:overworld".to_owned());
        let pos = match command.argument("pos") {
            Some(CommandArgumentValue::BlockPos(pos)) => *pos,
            _ => block_of(command.position),
        };
        report(host, &dimension, pos);
        Flow::Cancel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_probe_tests_the_block_in_the_named_dimension() {
        assert_eq!(
            probe_line("minecraft:the_nether", [10, -3, -200], "minecraft:fortress"),
            "execute in minecraft:the_nether positioned 10 -3 -200 if predicate wits:minecraft/fortress"
        );
    }

    #[test]
    fn a_position_is_floored_to_its_block() {
        assert_eq!(block_of([-0.25, 64.99, 3.5]), [-1, 64, 3]);
    }

    #[test]
    fn every_listed_structure_has_its_predicate_file() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/data/wits/predicate/minecraft");
        for structure in VANILLA_STRUCTURES {
            let file = std::fs::read_to_string(format!("{root}/{structure}.json"))
                .unwrap_or_else(|_| panic!("missing predicate for {structure}"));
            let json: serde_json::Value = serde_json::from_str(&file).unwrap();
            assert_eq!(json["type"], "minecraft:location_check");
            assert_eq!(
                json["predicate"]["structures"],
                format!("minecraft:{structure}")
            );
        }
    }
}
