//! QuarryPlus for Horizon: the Quarry and its Markers.
//!
//! Written from QuarryPlus' observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). Released under LGPL-3.0-only, as the original. What is
//! ported, and what a check against the NeoForge original still has to confirm, is in README.md.
//!
//! Three Markers in an L (one at the corner, one along x and one along z, up to 64 blocks away)
//! mark an area; a Quarry touching the corner marker digs the blocks between them, a layer at a
//! time from just below itself down to the bottom of the world, spending energy on every block
//! by its hardness ([`area::break_cost`]). The drops go into any inventory beside the Quarry,
//! and onto the ground when there is none with room.
pub mod area;

use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Compound, Direction, HostBlockStates, Hooks, Host, HostTransaction, ITEMS,
    ItemResource, ItemStack, LevelRef, LevelSide, ModEnergyBuffer, Registrar, RegistrationContext,
    ServerHost, Tag,
};

use crate::area::{Area, break_cost, find_area, skips};

pub const CAPACITY: i32 = 1_000_000;
pub const MAX_INSERT: i32 = 100_000;
/// Positions passed over (air and the like) in one tick before the quarry waits for the next.
pub const SKIPS_PER_TICK: usize = 256;
/// Ticks between two searches for markers while the quarry has no area.
pub const SEARCH_PERIOD: i32 = 20;

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

fn area_of(nbt: &Compound) -> Option<Area> {
    match nbt.get(&"area".into()) {
        Some(Tag::IntArray(a)) if a.len() == 4 => Some(Area { x0: a[0], x1: a[1], z0: a[2], z1: a[3] }),
        _ => None,
    }
}

const DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

fn beside(pos: [i32; 3], dir: Direction) -> [i32; 3] {
    let n = dir.normal();
    [pos[0] + n[0], pos[1] + n[1], pos[2] + n[2]]
}

fn is_marker(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> bool {
    level.block(host, pos).is_some_and(|b| b.id(host) == "quarryplus:marker")
}

/// Puts the drops in the inventories beside `pos`, and what does not fit on the ground above it.
fn deliver(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], drops: Vec<ItemStack>) {
    for stack in drops {
        let mut left = stack.count();
        let resource = ItemResource::of(&stack);
        for dir in DIRECTIONS {
            if left == 0 {
                break;
            }
            let Some(cap) = level.capability(host, &ITEMS, beside(pos, dir), Some(dir.opposite())) else {
                continue;
            };
            let went = host
                .transaction(|host, tx| Ok::<u32, ()>(cap.insert_any(host, tx, &resource, left)))
                .unwrap_or(0);
            left -= went.min(left);
        }
        if left > 0 {
            let mut rest = stack.clone();
            if rest.set_count(left).is_ok() {
                let _ = level.spawn_item(host, f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 1.0, f64::from(pos[2]) + 0.5, rest, 10);
            }
        }
    }
}

fn tick(host: &mut dyn ServerHost, level: LevelRef, event: &mut BlockEntityTickEvent<'_>) -> bool {
    let pos = event.pos;
    if int(&event.entity.nbt, "done") != 0 {
        return false;
    }
    let Some(area) = area_of(&event.entity.nbt) else {
        let phase = (int(&event.entity.nbt, "phase") + 1) % SEARCH_PERIOD;
        event.entity.nbt.insert("phase".into(), Tag::Int(phase));
        if phase != 0 {
            return true;
        }
        let corner = DIRECTIONS.iter().map(|&d| beside(pos, d)).find(|&p| is_marker(&*host, level, p));
        if let Some(area) = corner.and_then(|c| find_area(c, |p| is_marker(&*host, level, p))) {
            event.entity.nbt.insert("area".into(), Tag::IntArray(vec![area.x0, area.x1, area.z0, area.z1]));
            event.entity.nbt.insert("cursor".into(), Tag::Int(0));
        }
        return true;
    };
    let Some((min_y, _)) = level.height(&*host) else { return false };
    let top = pos[1] - 1;
    let total = area.cells() * i64::from((top - min_y + 1).max(0));
    let mut cursor = i64::from(int(&event.entity.nbt, "cursor"));
    let mut energy = int(&event.entity.nbt, "energy");
    let mut changed = false;
    for _ in 0..SKIPS_PER_TICK {
        if cursor >= total {
            event.entity.nbt.insert("done".into(), Tag::Int(1));
            changed = true;
            break;
        }
        let target = area.position(cursor, top);
        if !level.is_loaded(&*host, target) {
            break;
        }
        let Some(block) = level.block(&*host, target) else { break };
        let id = block.id(&*host);
        let hardness = level
            .block_state(&*host, target)
            .and_then(|s| host.block_states().destroy_speed(s))
            .unwrap_or(0.0);
        if skips(&id, hardness) {
            cursor += 1;
            changed = true;
            continue;
        }
        let cost = break_cost(hardness);
        if i64::from(energy) < cost {
            break;
        }
        let Ok(Some(drops)) = level.destroy_block(host, target, true, None, true) else { break };
        energy -= cost as i32;
        cursor += 1;
        changed = true;
        deliver(host, level, pos, drops);
        break;
    }
    if changed {
        event.entity.nbt.insert("cursor".into(), Tag::Int(cursor as i32));
        event.entity.nbt.insert("energy".into(), Tag::Int(energy));
    }
    changed
}

struct Quarry;

#[mod_api::horizon_part]
impl BlockPart for Quarry {}

#[mod_api::horizon_part]
impl BlockEntityPart for Quarry {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == "quarryplus:quarry").then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == "quarryplus:quarry" {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let Some(level) = host.level(host.dimension()) else { return };
        if !tick(host, level, event) {
            *event.reply = BlockEntityTickReply::Unchanged;
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let key = r.block_key("quarryplus:quarry");
        r.block_part(key, Quarry);
        r.block_energy(&key, ModEnergyBuffer {
            capacity: CAPACITY,
            max_insert: MAX_INSERT,
            max_extract: 0,
            insert_faces: [true; 6],
            extract_faces: [false; 6],
        });
        let kind = r.block_entity_type("quarry").blocks(&["quarryplus:quarry"]).build();
        let _ = r.block_entity_part(&kind, Quarry);
    }
}
