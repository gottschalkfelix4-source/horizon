//! XNet for Horizon: Net Cables, Connectors and the Controller that moves items, fluids and
//! energy between the blocks the connectors face.
//!
//! Written from XNet's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What is ported, and what a check against the NeoForge original
//! still has to confirm, is in README.md.
//!
//! A Controller finds its network: the Net Cables that touch it (and each other), and every
//! Connector hanging on one of them or on the Controller. Each Connector faces one block, moves
//! one kind of resource on one channel (a dye colour), and either extracts from what it faces or
//! inserts into it. Every [`PERIOD`] ticks the Controller moves what extractors can give to the
//! inserters of their channel, highest priority first ([`net::plan`]).
//!
//! The original sets a connector in the controller's screen. Here, an empty-hand click on a
//! Connector steps its mode (off, extract, insert), a sneaking click its kind, and a dye sets its
//! channel.
pub mod net;

use mod_api::{
    move_energy, move_resources, BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent,
    BlockEntityTickReply, BlockEntityTicksEvent, BlockPart, Compound, Direction, Hooks, Host,
    HostTransaction, InteractionResult, LevelRef, LevelSide, Registrar, RegistrationContext,
    ServerHost, Tag, Text, UseItemOnEvent, UseWithoutItemEvent, ENERGY, FLUIDS, ITEMS,
};

use crate::net::{Connector, Kind, Mode};

/// Ticks between two operations of a controller.
pub const PERIOD: i64 = 10;
/// Items, millibuckets or energy units one extractor moves per operation.
pub const ITEMS_PER_OP: u32 = 16;
pub const FLUID_PER_OP: u32 = 1_000;
pub const ENERGY_PER_OP: u32 = 1_000;
/// The most cables one network walk visits.
pub const NETWORK_LIMIT: usize = 512;

const DYES: [&str; 16] = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray",
    "cyan", "purple", "blue", "brown", "green", "red", "black",
];

const DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        Some(Tag::Byte(v)) => i32::from(*v),
        _ => 0,
    }
}

fn is_cable(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> bool {
    level.block(host, pos).is_some_and(|b| b.id(host) == "xnet:netcable")
}

fn connector_at(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<Connector> {
    if level.block(host, pos)?.id(host) != "xnet:connector" {
        return None;
    }
    let facing = net::dir_of(&level.block_property(host, pos, "facing")?)?;
    let nbt = host.block_entity(level, pos)?.data(host)?;
    Some(Connector {
        pos,
        facing,
        kind: Kind::from_index(int(&nbt, "kind")),
        mode: Mode::from_index(int(&nbt, "mode")),
        channel: int(&nbt, "channel").clamp(0, 15) as u8,
        priority: int(&nbt, "priority"),
    })
}

/// The connectors of the network of the controller at `controller`.
fn find_connectors(host: &dyn Host, level: LevelRef, controller: [i32; 3]) -> Vec<Connector> {
    let starts: Vec<[i32; 3]> = (0..6).map(|d| net::offset(controller, d)).collect();
    let cables = net::cables(&starts, |p| is_cable(host, level, p), NETWORK_LIMIT);
    let mut nodes = cables;
    nodes.push(controller);
    let mut out: Vec<Connector> = Vec::new();
    for node in nodes {
        for d in 0..6 {
            let at = net::offset(node, d);
            if out.iter().any(|c| c.pos == at) {
                continue;
            }
            if let Some(c) = connector_at(host, level, at) {
                if net::attached(c.pos, c.facing, node) {
                    out.push(c);
                }
            }
        }
    }
    out
}

fn move_between(host: &mut dyn ServerHost, level: LevelRef, from: &Connector, to: &Connector) -> u32 {
    let side = |c: &Connector| Some(DIRECTIONS[net::opposite(c.facing)]);
    match from.kind {
        Kind::Items => {
            let (Some(a), Some(b)) = (
                level.capability(host, &ITEMS, from.target(), side(from)),
                level.capability(host, &ITEMS, to.target(), side(to)),
            ) else {
                return 0;
            };
            host.transaction(|host, tx| move_resources(host, &a, &b, |_| true, ITEMS_PER_OP, tx))
                .unwrap_or(0)
        }
        Kind::Fluids => {
            let (Some(a), Some(b)) = (
                level.capability(host, &FLUIDS, from.target(), side(from)),
                level.capability(host, &FLUIDS, to.target(), side(to)),
            ) else {
                return 0;
            };
            host.transaction(|host, tx| move_resources(host, &a, &b, |_| true, FLUID_PER_OP, tx))
                .unwrap_or(0)
        }
        Kind::Energy => {
            let (Some(a), Some(b)) = (
                level.capability(host, &ENERGY, from.target(), side(from)),
                level.capability(host, &ENERGY, to.target(), side(to)),
            ) else {
                return 0;
            };
            host.transaction(|host, tx| move_energy(host, &a, &b, ENERGY_PER_OP, tx))
                .unwrap_or(0)
        }
    }
}

fn run_controller(host: &mut dyn ServerHost, level: LevelRef, controller: [i32; 3]) {
    let conns = find_connectors(&*host, level, controller);
    for route in net::plan(&conns) {
        let from = &conns[route.from];
        for &to in &route.to {
            if move_between(host, level, from, &conns[to]) > 0 {
                break;
            }
        }
    }
}

struct Xnet;

fn tell(host: &mut dyn ServerHost, player: u128, key: &str, args: Vec<Text>) {
    if let Some(p) = host.player(player) {
        let _ = p.action_bar(host, Text::translate(key, args));
    }
}

fn patch(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], key: &str, value: i32) {
    if let Some(entity) = host.block_entity(level, pos) {
        let mut p = Compound::new();
        p.insert(key.into(), Tag::Int(value));
        let _ = entity.merge_data(host, p);
    }
}

#[mod_api::horizon_part]
impl BlockPart for Xnet {
    fn use_item_on(&self, host: &mut dyn ServerHost, event: &mut UseItemOnEvent<'_>) -> Option<InteractionResult> {
        let click = event.click;
        let level = host.level(host.dimension())?;
        let conn = connector_at(&*host, level, click.pos)?;
        let id = click.stack?.item().id(&*host);
        let colour = DYES.iter().position(|d| id == format!("minecraft:{d}_dye"))?;
        if conn.channel as usize != colour {
            patch(host, level, click.pos, "channel", colour as i32);
        }
        tell(host, click.player, "xnet.channel", vec![Text::literal(&colour.to_string())]);
        Some(InteractionResult::SuccessServer)
    }

    fn use_without_item(&self, host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
        let click = event.click;
        let level = host.level(host.dimension())?;
        let conn = connector_at(&*host, level, click.pos)?;
        let sneaking = host.player(click.player)?.is_secondary_use_active(&*host)?;
        if sneaking {
            let kind = Kind::from_index((conn.kind.index() + 1) % 3);
            patch(host, level, click.pos, "kind", kind.index());
            tell(host, click.player, &format!("xnet.kind.{}", kind.index()), Vec::new());
        } else {
            let mode = conn.mode.next();
            patch(host, level, click.pos, "mode", mode.index());
            tell(host, click.player, &format!("xnet.mode.{}", mode.index()), Vec::new());
        }
        Some(InteractionResult::SuccessServer)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for Xnet {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        match event.block {
            "xnet:connector" | "xnet:controller" => Some(Compound::new()),
            _ => None,
        }
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == "xnet:controller" {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        *event.reply = BlockEntityTickReply::Unchanged;
        let Some(level) = host.level(host.dimension()) else { return };
        let time = level.game_time(&*host).unwrap_or(0);
        // Spread the controllers over the period by where they stand.
        let phase = i64::from(event.pos[0] ^ event.pos[1] ^ event.pos[2]).rem_euclid(PERIOD);
        if time.rem_euclid(PERIOD) != phase {
            return;
        }
        run_controller(host, level, event.pos);
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for id in ["xnet:netcable", "xnet:connector", "xnet:controller"] {
            let key = r.block_key(id);
            r.block_part(key, Xnet);
        }
        let kind = r
            .block_entity_type("xnet")
            .blocks(&["xnet:connector", "xnet:controller"])
            .build();
        let _ = r.block_entity_part(&kind, Xnet);
    }
}
