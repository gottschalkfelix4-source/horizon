//! Laser Bridges for Horizon: an emitter that draws a walkable bridge of light.
//!
//! Written from Laser Bridges' public description, never from its code (docs/modpacks.md,
//! "Licences"). The mechanics are the port's reading of that description; README.md lists what a
//! check against the NeoForge original still has to confirm.
//!
//! While powered by redstone the emitter (`laserbridges:laser_emitter`, the dropper's `facing`)
//! fills the air in front of it with `laser_beam` blocks up to 32 long, and removes them when the
//! power ends. A `laser_receiver` the beam ends on lights (`lit`) and gives a redstone signal.
//! A beam block with no emitter behind it removes itself on a random tick.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, BlockSignalEvent, BlockUpdateFlags, Compound, Direction,
    Flow, Hooks, Host, IsSignalSourceEvent, LevelHost, LevelRef, LevelSide, LevelWriteHost,
    RandomTickEvent, Registrar, RegistrationContext, ServerHost,
};

pub mod rules;
use rules::*;

const EMITTER: &str = "laserbridges:laser_emitter";
const RECEIVER: &str = "laserbridges:laser_receiver";
const BEAM: &str = "laserbridges:laser_beam";

fn direction_named(name: &str) -> Option<Direction> {
    Some(match name {
        "north" => Direction::North,
        "south" => Direction::South,
        "east" => Direction::East,
        "west" => Direction::West,
        "up" => Direction::Up,
        "down" => Direction::Down,
        _ => return None,
    })
}

fn name_of(dir: Direction) -> &'static str {
    match dir {
        Direction::North => "north",
        Direction::South => "south",
        Direction::East => "east",
        Direction::West => "west",
        Direction::Up => "up",
        Direction::Down => "down",
    }
}

fn axis_of(dir: Direction) -> &'static str {
    match dir {
        Direction::Up | Direction::Down => "y",
        Direction::North | Direction::South => "z",
        _ => "x",
    }
}

fn step(pos: [i32; 3], dir: Direction, n: i32) -> [i32; 3] {
    let d = dir.normal();
    [pos[0] + d[0] * n, pos[1] + d[1] * n, pos[2] + d[2] * n]
}

fn block_id(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<String> {
    level.block(host, pos).map(|k| k.id(host))
}

fn cell(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Cell {
    match block_id(host, level, pos).as_deref() {
        Some("minecraft:air" | "minecraft:cave_air" | "minecraft:void_air") => Cell::Air,
        Some(BEAM) => Cell::Beam,
        Some(RECEIVER) => Cell::Receiver,
        _ => Cell::Solid,
    }
}

struct Emitter;

#[mod_api::horizon_part]
impl BlockPart for Emitter {}

#[mod_api::horizon_part]
impl BlockEntityPart for Emitter {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == EMITTER).then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == EMITTER {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        *event.reply = BlockEntityTickReply::Unchanged;
        let pos = event.pos;
        let Some(level) = host.level(host.dimension()) else { return };
        // every 4 ticks is enough for a bridge
        if level.game_time(&*host).is_none_or(|t| (t + i64::from(pos[0] ^ pos[2])).rem_euclid(4) != 0) {
            return;
        }
        if block_id(&*host, level, pos).as_deref() != Some(EMITTER) {
            return;
        }
        let Some(facing) = level
            .block_property(&*host, pos, "facing")
            .and_then(|f| direction_named(&f))
        else {
            return;
        };
        let powered = level.has_neighbor_signal(&*host, pos);
        let line = (1..).map(|n| cell(&*host, level, step(pos, facing, n)));
        let made = if powered { bridge(line) } else { Bridge { length: 0, receiver: false } };
        let beam = host.block_state_id(&format!("{BEAM}[axis={}]", axis_of(facing)));
        let air = host.block_state_id("minecraft:air");
        let (Some(beam), Some(air)) = (beam, air) else { return };
        let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
        // place the beam, and clear what an older, longer bridge left
        for n in 1..=RANGE as i32 + 1 {
            let at = step(pos, facing, n);
            match cell(&*host, level, at) {
                Cell::Air if (n as usize) <= made.length => {
                    let _ = level.set_block(host, at, beam, flags);
                }
                Cell::Beam if (n as usize) > made.length => {
                    let _ = level.set_block(host, at, air, flags);
                }
                Cell::Beam | Cell::Air => {}
                _ => break,
            }
        }
        // the receiver the beam ends on
        let end = step(pos, facing, made.length as i32 + 1);
        if block_id(&*host, level, end).as_deref() == Some(RECEIVER) {
            let lit = if made.receiver { "true" } else { "false" };
            host.set_block_property(end, "lit", lit);
        }
    }
}

struct Receiver;

#[mod_api::horizon_part]
impl BlockPart for Receiver {
    fn block_signal(&self, level: &dyn LevelHost, event: &mut BlockSignalEvent) {
        let lit = level.state_string(event.state).is_some_and(|s| s.contains("lit=true"));
        event.signal = if lit { 15 } else { 0 };
    }

    fn is_signal_source(&self, _: &dyn LevelHost, event: &mut IsSignalSourceEvent) {
        event.source = true;
    }
}

struct Beam;

#[mod_api::horizon_part]
impl BlockPart for Beam {
    /// A beam block with no lit emitter behind it on its axis goes out.
    fn random_tick(&self, level: &mut dyn LevelWriteHost, event: &mut RandomTickEvent) -> Flow {
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        let Some(text) = level.state_string(event.state) else { return Flow::Continue };
        let dirs: &[Direction] = if text.contains("axis=y") {
            &[Direction::Up, Direction::Down]
        } else if text.contains("axis=z") {
            &[Direction::North, Direction::South]
        } else {
            &[Direction::East, Direction::West]
        };
        for dir in dirs {
            for n in 1..=RANGE as i32 + 1 {
                let at = step(pos, *dir, n);
                let Some(state) = level.block_state(at) else { break };
                match level.state_block(state) {
                    Some(BEAM) => continue,
                    Some(EMITTER) => {
                        // an emitter faces the beam when its facing points this way
                        let faces = level
                            .state_string(state)
                            .is_some_and(|s| s.contains(&format!("facing={}", name_of(dir.opposite()))));
                        if faces {
                            return Flow::Continue;
                        }
                        break;
                    }
                    _ => break,
                }
            }
        }
        if let Some(air) = level.block_state_id("minecraft:air") {
            level.set_block(pos, air, BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS);
        }
        Flow::Continue
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let kind = r.block_entity_type("laser_emitter").blocks(&[EMITTER]).build();
        let _ = r.block_entity_part(&kind, Emitter);
        let receiver = r.block_key(RECEIVER);
        r.block_part(receiver, Receiver);
        let beam = r.block_key(BEAM);
        r.block_part(beam, Beam);
    }
}
