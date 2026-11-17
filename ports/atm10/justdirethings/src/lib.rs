//! Just Dire Things for Horizon: the goo that turns blocks into raw ores, the coal generator, and the
//! tools, armor, fuels and machines of the original as data.
//!
//! Written from the mod's data files and its observed behaviour, never from its code
//! (docs/modpacks.md, "Licences"). Items and blocks are data (`data/justdirethings/horizon`, written
//! by `gen_data.py`, fuels as `minecraft:cooking_fuel` with the original's burn times). The goo reads
//! the original's `justdirethings:goospread` recipes; README.md lists what is ported and what is not.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, BlockState, BlockUpdateFlags, Compound, Direction, ENERGY,
    Hooks, Host, HostBlockStates, HostTransaction, ItemStack, LevelRef, LevelSide, ModContainer,
    ModEnergyBuffer, Registrar, RegistrationContext, ServerHost, StateConstraint, Tag,
};

pub mod rules;
use rules::*;

#[cfg(test)]
mod data_check;

const GENERATOR: &str = "justdirethings:generatort1";
const GENERATOR_SLOTS: usize = 5;

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

fn long(nbt: &Compound, key: &str) -> i64 {
    match nbt.get(&key.into()) {
        Some(Tag::Long(v)) => *v,
        Some(Tag::Int(v)) => i64::from(*v),
        _ => 0,
    }
}

fn set_int(nbt: &mut Compound, key: &str, v: i32) {
    nbt.insert(key.into(), Tag::Int(v));
}

fn set_long(nbt: &mut Compound, key: &str, v: i64) {
    nbt.insert(key.into(), Tag::Long(v));
}

fn offset(pos: [i32; 3], dir: Direction) -> [i32; 3] {
    let n = dir.normal();
    [pos[0] + n[0], pos[1] + n[1], pos[2] + n[2]]
}

fn block_id(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<String> {
    level.block(host, pos).map(|k| k.id(host))
}

fn is_machine(id: &str) -> bool {
    id == GENERATOR || goo_tier(id).is_some()
}

struct Machine;

#[mod_api::horizon_part]
impl BlockPart for Machine {}

#[mod_api::horizon_part]
impl BlockEntityPart for Machine {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        if !is_machine(event.block) {
            return None;
        }
        let mut nbt = Compound::new();
        set_int(&mut nbt, "energy", 0);
        set_int(&mut nbt, "burn", 0);
        set_int(&mut nbt, "side", 0);
        set_long(&mut nbt, "progress", 0);
        Some(nbt)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if is_machine(event.block) {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        let Some(id) = block_id(&*host, level, pos) else {
            return;
        };
        let mut nbt = event.entity.nbt.clone();
        let mut items = event.entity.items.clone();
        if let Some(tier) = goo_tier(&id) {
            tick_goo(host, level, pos, tier, &mut nbt);
        } else if id == GENERATOR {
            tick_generator(host, level, pos, &mut nbt, &mut items);
        }
        if nbt == event.entity.nbt && items == event.entity.items {
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        }
        event.entity.nbt = nbt;
        event.entity.items = items;
    }
}

fn goo_recipes(host: &dyn ServerHost) -> Vec<Goo> {
    host.recipes_of_type("justdirethings:goospread")
        .iter()
        .filter_map(|r| parse_goo(&r.json))
        .collect()
}

/// A goo block works on the neighbour its recipes name, and replaces it when the time is up.
fn tick_goo(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    tier: u32,
    nbt: &mut Compound,
) {
    let neighbours: [Option<String>; 6] =
        std::array::from_fn(|i| block_id(&*host, level, offset(pos, Direction::ALL[i])));
    if neighbours
        .iter()
        .all(|n| n.as_deref().is_none_or(|b| b == "minecraft:air"))
    {
        return;
    }
    let recipes = goo_recipes(host);
    let mut state = GooState {
        side: int(nbt, "side").clamp(0, 5) as usize,
        progress: long(nbt, "progress"),
    };
    let done = goo_tick(&mut state, &neighbours, &recipes, tier).map(|(side, r)| (side, r.clone()));
    set_int(nbt, "side", state.side as i32);
    set_long(nbt, "progress", state.progress);
    let Some((side, recipe)) = done else {
        return;
    };
    let target = offset(pos, Direction::ALL[side]);
    let Some(key) = host.block_key(&recipe.output) else {
        return;
    };
    let constraints: Vec<StateConstraint> = recipe
        .output_properties
        .iter()
        .map(|(k, v)| StateConstraint::new(k.as_str(), v.as_str()))
        .collect();
    let Ok(new_state) = BlockState::of(&host.block_states(), key, &constraints) else {
        return;
    };
    let _ = level.set_block(host, target, new_state, BlockUpdateFlags::CLIENTS);
}

/// The coal generator burns the fuel in its slots (burn times are the items' own `cooking_fuel`) for
/// energy and gives it to its neighbours.
fn tick_generator(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    nbt: &mut Compound,
    items: &mut Vec<Option<ItemStack>>,
) {
    if items.len() < GENERATOR_SLOTS {
        items.resize(GENERATOR_SLOTS, None);
    }
    let stored = int(nbt, "energy").max(0);
    let (made, burn, wants) = generator_tick(int(nbt, "burn"), GENERATOR_CAPACITY - stored);
    let mut burn = burn;
    if wants {
        let slot = items.iter().position(|s| {
            s.as_ref()
                .is_some_and(|s| host.item_burn_time(s.item()).unwrap_or(0) > 0)
        });
        if let Some(slot) = slot
            && let Some(stack) = items[slot].take()
        {
            burn = host.item_burn_time(stack.item()).unwrap_or(0);
            let remainder = host.item_crafting_remainder(&stack);
            items[slot] = stack.shrink(1).or(remainder);
        }
    }
    set_int(nbt, "burn", burn);
    let mut energy = stored + made;
    energy -= push(host, level, pos, energy);
    set_int(nbt, "energy", energy);
}

fn push(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], available: i32) -> i32 {
    let mut left = available.clamp(0, GENERATOR_PUSH) as u32;
    let mut given = 0u32;
    for dir in Direction::ALL {
        if left == 0 {
            break;
        }
        let Some(cap) = level.capability(host, &ENERGY, offset(pos, dir), Some(dir.opposite()))
        else {
            continue;
        };
        let moved = host
            .transaction(|host, tx| Ok::<u32, ()>(cap.insert(host, tx, left)))
            .unwrap_or(0);
        left -= moved.min(left);
        given += moved;
    }
    given as i32
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let mut names = vec![GENERATOR.to_owned()];
        names.extend((1..=4).map(|t| format!("justdirethings:gooblock_tier{t}")));
        let generator = r.block_key(GENERATOR);
        r.block_energy(
            &generator,
            ModEnergyBuffer {
                capacity: GENERATOR_CAPACITY,
                max_insert: 0,
                max_extract: GENERATOR_PUSH,
                insert_faces: [false; 6],
                extract_faces: [true; 6],
            },
        );
        let every: Vec<usize> = (0..GENERATOR_SLOTS).collect();
        r.block_container(
            &generator,
            ModContainer {
                slots: GENERATOR_SLOTS,
                faces: std::array::from_fn(|_| every.clone()),
                inputs: every.clone(),
                outputs: vec![],
                menu: Some("minecraft:hopper".into()),
                slot_limit: 0,
            },
        );
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let built = r
            .block_entity_type("justdirethings:machine")
            .blocks(&names)
            .build();
        let _ = r.block_entity_part(&built, Machine);
        // the types of the original's recipes the port reads or only carries
        r.register_recipe_type("justdirethings:goospread");
        r.register_raw_recipe_serializer("justdirethings:goospread", "justdirethings:goospread");
    }
}
