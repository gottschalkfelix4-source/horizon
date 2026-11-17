//! Mekanism Generators for Horizon: heat, solar, advanced solar, wind and bio generators.
//!
//! Written from the mod's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). Energy lives in each block entity's saved `energy` (the
//! game's `ModEnergyBuffer`); a generator gives to adjacent energy blocks and the Mekanism
//! port's universal cables draw from it.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Compound, Direction, ENERGY, Hooks, Host, HostTransaction,
    ItemStack, LevelRef, LevelSide, ModContainer, ModEnergyBuffer, Registrar,
    RegistrationContext, ServerHost, Tag,
};

pub mod rules;
use rules::*;

/// Fuel slots, as many as the hopper menu shows.
const SLOTS: usize = 5;

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

fn set_int(nbt: &mut Compound, key: &str, v: i32) {
    nbt.insert(key.into(), Tag::Int(v));
}

fn offset(pos: [i32; 3], dir: Direction) -> [i32; 3] {
    let n = dir.normal();
    [pos[0] + n[0], pos[1] + n[1], pos[2] + n[2]]
}

/// Offers up to `max` of `available` to the neighbours that take energy: how much they took.
fn push_to_neighbours(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], available: i32, max: i32) -> i32 {
    let mut left = available.min(max).max(0) as u32;
    let mut given = 0u32;
    for dir in Direction::ALL {
        if left == 0 {
            break;
        }
        let Some(cap) = level.capability(host, &ENERGY, offset(pos, dir), Some(dir.opposite())) else {
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

fn tick_generator(
    host: &mut dyn ServerHost,
    level: LevelRef,
    g: Gen,
    pos: [i32; 3],
    nbt: &mut Compound,
    items: &mut Vec<Option<ItemStack>>,
) {
    let held = int(nbt, "energy").max(0);
    let room = g.buffer() - held;
    let made = match g {
        Gen::Solar | Gen::AdvancedSolar => {
            let above = [pos[0], pos[1] + 1, pos[2]];
            let day = level.get_sky_darken(&*host).is_some_and(|d| d < 4);
            let sky = level.can_see_sky(&*host, above).unwrap_or(false);
            let rain = level.is_raining_at(&*host, above).unwrap_or(false);
            solar_output(g, day, sky, rain)
        }
        Gen::Wind => wind_output(pos[1]),
        Gen::Heat | Gen::Bio => {
            if items.len() < SLOTS {
                items.resize(SLOTS, None);
            }
            let mut burn = Burn { left: int(nbt, "burn") };
            let made = burn.tick(g, room, || {
                let slot = items.iter().position(|s| {
                    s.as_ref().is_some_and(|s| {
                        let vanilla = host.item_burn_time(s.item()).unwrap_or(0);
                        burn_ticks(g, &s.item().id(&*host), vanilla) > 0
                    })
                })?;
                let stack = items[slot].take()?;
                let vanilla = host.item_burn_time(stack.item()).unwrap_or(0);
                let ticks = burn_ticks(g, &stack.item().id(&*host), vanilla);
                let remainder = host.item_crafting_remainder(&stack);
                items[slot] = stack.shrink(1).or(remainder);
                Some(ticks)
            });
            set_int(nbt, "burn", burn.left);
            made
        }
    };
    let mut e = (held + made.min(room.max(0))).min(g.buffer());
    e -= push_to_neighbours(host, level, pos, e, g.rate());
    set_int(nbt, "energy", e);
    // the furnace template's `lit` shows a burner making energy
    if g.burns() {
        let lit = i32::from(made > 0);
        if int(nbt, "lit") != lit {
            set_int(nbt, "lit", lit);
            let _ = host.set_block_property(pos, "lit", if lit == 1 { "true" } else { "false" });
        }
    }
}

/// One part for every generator block entity.
struct Generators;

#[mod_api::horizon_part]
impl BlockPart for Generators {}

#[mod_api::horizon_part]
impl BlockEntityPart for Generators {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        Gen::from_id(event.block).map(|_| {
            let mut nbt = Compound::new();
            set_int(&mut nbt, "energy", 0);
            nbt
        })
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if Gen::from_id(event.block).is_some() {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let Some(level) = host.level(host.dimension()) else { return };
        let Some(id) = level.block(&*host, pos).map(|k| k.id(&*host)) else { return };
        let Some(g) = Gen::from_id(&id) else { return };
        let mut nbt = event.entity.nbt.clone();
        let mut items = event.entity.items.clone();
        tick_generator(host, level, g, pos, &mut nbt, &mut items);
        if nbt == event.entity.nbt && items == event.entity.items {
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        }
        event.entity.nbt = nbt;
        event.entity.items = items;
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let ids: Vec<String> = Gen::ALL.iter().map(|g| g.id()).collect();
        for g in Gen::ALL {
            let key = r.block_key(&g.id());
            r.block_energy(
                &key,
                ModEnergyBuffer {
                    capacity: g.buffer(),
                    max_insert: 0,
                    max_extract: g.rate(),
                    insert_faces: [false; 6],
                    extract_faces: [true; 6],
                },
            );
            if g.burns() {
                r.block_container(
                    &key,
                    ModContainer {
                        slots: SLOTS,
                        faces: std::array::from_fn(|_| (0..SLOTS).collect()),
                        inputs: (0..SLOTS).collect(),
                        outputs: vec![],
                        menu: Some("minecraft:hopper".into()),
                    },
                );
            }
        }
        let names: Vec<&str> = ids.iter().map(String::as_str).collect();
        let built = r.block_entity_type("generator").blocks(&names).build();
        let _ = r.block_entity_part(&built, Generators);
    }
}
