//! The digital miner: mines a square of radius 10 around itself, from the block under it down
//! to the bottom of the level, layer by layer, for energy, and keeps what drops in its 27 slots
//! (what does not fit lies on the ground above it). A hopper or a pipe can empty it. Stand-in:
//! no filters, no silk touch, no radius or offset settings, no screen (vanilla's 9x3 menu shows
//! the slots), no visible area. See README.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Compound, Host, HostBlockStates, ItemStack, LevelRef, LevelSide, ModContainer,
    ModEnergyBuffer, Registrar, ServerHost, Tag,
};

pub const BLOCK: &str = "mekanism:digital_miner";
pub const RADIUS: i32 = 10;
pub const SLOTS: usize = 27;
pub const CAPACITY: i32 = 100_000;
/// Ticks between two blocks.
pub const COOLDOWN: i32 = 10;
/// Cells looked at in one tick while searching for something to mine.
const LOOK: i32 = 64;

/// FE to mine a block of `hardness`.
pub fn cost(hardness: f32) -> i32 {
    400 + (hardness.max(0.0) * 400.0).round() as i32
}

/// Whether the miner leaves this alone: air, fluids, bedrock-like blocks (negative hardness) and itself.
pub fn skips(id: &str, hardness: f32) -> bool {
    hardness < 0.0
        || matches!(id, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" | "minecraft:water" | "minecraft:lava")
        || id == BLOCK
}

/// The block the `cursor`th cell is, going down layer by layer from `top`, row by row in each.
pub fn position(origin: [i32; 3], top: i32, cursor: i64) -> [i32; 3] {
    let side = i64::from(2 * RADIUS + 1);
    let layer = cursor / (side * side);
    let cell = cursor % (side * side);
    [
        origin[0] - RADIUS + (cell % side) as i32,
        top - layer as i32,
        origin[2] - RADIUS + (cell / side) as i32,
    ]
}

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

/// Puts `stack` into `items`, merging into stacks of the same item first: what is left over.
fn store(host: &dyn Host, items: &mut [Option<ItemStack>], mut stack: ItemStack) -> Option<ItemStack> {
    let max = host.max_stack_size(stack.item()).unwrap_or(64);
    for slot in items.iter_mut() {
        let Some(held) = slot else { continue };
        if !held.is_stackable_with(&stack) || held.count() >= max {
            continue;
        }
        let moved = (max - held.count()).min(stack.count());
        if held.grow(moved).is_err() {
            continue;
        }
        match stack.shrink(moved) {
            Some(rest) => stack = rest,
            None => return None,
        }
    }
    for slot in items.iter_mut() {
        if slot.is_none() {
            *slot = Some(stack);
            return None;
        }
    }
    Some(stack)
}

fn tick(host: &mut dyn ServerHost, level: LevelRef, event: &mut BlockEntityTickEvent<'_>) -> bool {
    let pos = event.pos;
    let mut nbt = event.entity.nbt.clone();
    let mut items = event.entity.items.clone();
    items.resize(SLOTS, None);
    let cooldown = int(&nbt, "cooldown");
    if cooldown > 0 {
        nbt.insert("cooldown".into(), Tag::Int(cooldown - 1));
    } else if let Some((min_y, _)) = level.height(&*host) {
        let top = pos[1] - 1;
        let side = i64::from(2 * RADIUS + 1);
        let total = side * side * i64::from((top - min_y + 1).max(0));
        let mut cursor = i64::from(int(&nbt, "cursor"));
        let mut energy = int(&nbt, "energy");
        for _ in 0..LOOK {
            if cursor >= total {
                break;
            }
            let target = position(pos, top, cursor);
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
                continue;
            }
            let price = cost(hardness);
            if energy < price {
                break;
            }
            let Ok(Some(drops)) = level.destroy_block(host, target, true, None, true) else { break };
            energy -= price;
            cursor += 1;
            for drop in drops {
                if let Some(rest) = store(&*host, &mut items, drop) {
                    let _ = level.spawn_item(
                        host,
                        f64::from(pos[0]) + 0.5,
                        f64::from(pos[1]) + 1.0,
                        f64::from(pos[2]) + 0.5,
                        rest,
                        10,
                    );
                }
            }
            nbt.insert("cooldown".into(), Tag::Int(COOLDOWN - 1));
            break;
        }
        nbt.insert("cursor".into(), Tag::Int(cursor as i32));
        nbt.insert("energy".into(), Tag::Int(energy));
    }
    if nbt == event.entity.nbt && items == event.entity.items {
        return false;
    }
    event.entity.nbt = nbt;
    event.entity.items = items;
    true
}

struct Miner;

#[mod_api::horizon_part]
impl BlockPart for Miner {}

#[mod_api::horizon_part]
impl BlockEntityPart for Miner {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == BLOCK).then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == BLOCK {
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

pub fn register(r: &mut dyn Registrar) {
    let key = r.block_key(BLOCK);
    r.block_energy(
        &key,
        ModEnergyBuffer {
            capacity: CAPACITY,
            max_insert: 10_000,
            max_extract: 0,
            insert_faces: [true; 6],
            extract_faces: [false; 6],
        },
    );
    // hoppers and pipes empty it; nothing goes in
    r.block_container(
        &key,
        ModContainer {
            slots: SLOTS,
            faces: std::array::from_fn(|_| (0..SLOTS).collect()),
            inputs: vec![],
            outputs: (0..SLOTS).collect(),
            menu: Some("minecraft:generic_9x3".into()),
        },
    );
    let built = r.block_entity_type("digital_miner").blocks(&[BLOCK]).build();
    let _ = r.block_entity_part(&built, Miner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_miner_walks_each_layer_row_by_row_then_goes_down() {
        let origin = [100, 70, -20];
        assert_eq!(position(origin, 69, 0), [90, 69, -30]);
        assert_eq!(position(origin, 69, 20), [110, 69, -30]);
        assert_eq!(position(origin, 69, 21), [90, 69, -29]);
        assert_eq!(position(origin, 69, 441), [90, 68, -30]);
        assert_eq!(position(origin, 69, 440), [110, 69, -10]);
    }

    #[test]
    fn it_leaves_fluids_bedrock_and_itself_alone() {
        assert!(skips("minecraft:water", 100.0));
        assert!(skips("minecraft:bedrock", -1.0));
        assert!(skips(BLOCK, 3.5));
        assert!(!skips("minecraft:stone", 1.5));
        assert_eq!(cost(1.5), 1_000);
    }
}
