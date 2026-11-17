//! Create Additions for Horizon: an electric motor, an alternator, an accumulator, connectors, the
//! creative generator and the Tesla coil that charges items by the original's recipes.
//!
//! Written from the mod's data files and its observed behaviour, never from its code
//! (docs/modpacks.md, "Licences"). Blocks and items are data (`data/createaddition/horizon`, written by
//! `gen_data.py`). Energy lives in the block entity's saved `energy` (the game's `ModEnergyBuffer`), so
//! other mods' cables and machines reach it through `ENERGY`. Rotation is not in this crate: Create's
//! port keeps its own network, so the motor and alternator exchange speed through the `speed_x1000`
//! field of their saved data (see README.md).
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Compound, Direction, ENERGY, Hooks, Host, HostTransaction,
    InteractionResult, ItemStack, LevelRef, LevelSide, ModContainer, ModEnergyBuffer, Registrar,
    RegistrationContext, ServerHost, Tag, UseWithoutItemEvent,
};

pub mod rules;

#[cfg(test)]
mod data_check;
use rules::*;

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

/// Offers up to `max` of `available` to the neighbours `accept` lets through: how much they took.
fn push_to_neighbours(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    available: i32,
    max: i32,
    accept: impl Fn(&dyn Host, [i32; 3]) -> bool,
) -> i32 {
    let mut left = available.min(max).max(0) as u32;
    let mut given = 0u32;
    for dir in Direction::ALL {
        if left == 0 {
            break;
        }
        let n = offset(pos, dir);
        if !accept(&*host, n) {
            continue;
        }
        let Some(cap) = level.capability(host, &ENERGY, n, Some(dir.opposite())) else {
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

/// One part for every block entity of the mod; the block's id says which kind it is.
struct Machine;

#[mod_api::horizon_part]
impl BlockPart for Machine {
    /// An empty hand on the motor steps its speed.
    fn use_without_item(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseWithoutItemEvent<'_>,
    ) -> Option<InteractionResult> {
        let level = host.level(host.dimension())?;
        let pos = event.click.pos;
        if parse_block(&block_id(&*host, level, pos)?) != Some(Kind::Motor) {
            return None;
        }
        let entity = host.block_entity(level, pos)?;
        let data = entity.data(&*host)?;
        let next = next_motor_speed(int(&data, "rpm"));
        let mut patch = Compound::new();
        set_int(&mut patch, "rpm", next);
        let _ = entity.merge_data(host, patch);
        let player = host.player(event.click.player)?;
        let line = mod_api::Text::translate(
            "message.createaddition.motor_speed",
            vec![mod_api::Text::literal(next.to_string())],
        );
        let _ = player.action_bar(host, line);
        Some(InteractionResult::SuccessServer)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for Machine {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        let kind = parse_block(event.block)?;
        let mut nbt = Compound::new();
        set_int(&mut nbt, "energy", 0);
        match kind {
            Kind::Motor => set_int(&mut nbt, "rpm", MOTOR_DEFAULT_RPM),
            Kind::Alternator => set_int(&mut nbt, "speed_x1000", 0),
            Kind::Coil => set_long(&mut nbt, "progress", 0),
            _ => {}
        }
        Some(nbt)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if parse_block(event.block).is_some() {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        let Some(kind) = block_id(&*host, level, pos).and_then(|id| parse_block(&id)) else {
            return;
        };
        let mut nbt = event.entity.nbt.clone();
        let mut items = event.entity.items.clone();
        let stored = int(&nbt, "energy").max(0);
        match kind {
            Kind::Creative => {
                let given = push_to_neighbours(
                    host,
                    level,
                    pos,
                    CREATIVE_OUTPUT,
                    CREATIVE_OUTPUT,
                    |_, _| true,
                );
                let _ = given;
            }
            Kind::Accumulator => {
                let given =
                    push_to_neighbours(host, level, pos, stored, ACCUMULATOR_TRANSFER, |h, n| {
                        block_id(h, level, n).and_then(|id| parse_block(&id))
                            != Some(Kind::Accumulator)
                    });
                set_int(&mut nbt, "energy", stored - given);
            }
            Kind::Connector | Kind::LargeConnector => {
                let rate = if kind == Kind::Connector {
                    CONNECTOR_TRANSFER
                } else {
                    LARGE_CONNECTOR_TRANSFER
                };
                let given = push_to_neighbours(host, level, pos, stored, rate, |_, _| true);
                set_int(&mut nbt, "energy", stored - given);
            }
            Kind::Motor => {
                let rpm = int(&nbt, "rpm");
                let draw = motor_draw(rpm);
                let running = stored >= draw && draw > 0;
                set_int(
                    &mut nbt,
                    "energy",
                    if running { stored - draw } else { stored },
                );
                set_int(
                    &mut nbt,
                    "speed_x1000",
                    if running { rpm * 1000 } else { 0 },
                );
            }
            Kind::Alternator => {
                // Until Create's network can turn it, a running motor beside it does.
                let mut speed = int(&nbt, "speed_x1000");
                for dir in Direction::ALL {
                    let n = offset(pos, dir);
                    if block_id(&*host, level, n).and_then(|id| parse_block(&id))
                        != Some(Kind::Motor)
                    {
                        continue;
                    }
                    let turned = host
                        .block_entity(level, n)
                        .and_then(|e| e.data(&*host))
                        .map_or(0, |d| int(&d, "speed_x1000"));
                    speed = speed.max(turned);
                }
                let made = alternator_output(speed / 1000);
                let e = (stored + made).min(ALTERNATOR_CAPACITY);
                let given = push_to_neighbours(host, level, pos, e, ALTERNATOR_CAPACITY, |h, n| {
                    block_id(h, level, n).and_then(|id| parse_block(&id)) != Some(Kind::Motor)
                });
                set_int(&mut nbt, "energy", e - given);
            }
            Kind::Coil => tick_coil(host, &mut nbt, &mut items),
        }
        if nbt == event.entity.nbt && items == event.entity.items {
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        }
        event.entity.nbt = nbt;
        event.entity.items = items;
    }
}

/// The coil draws what the first chargeable item takes, up to its rate, and swaps it for the result in
/// the output slot when the item's energy is full.
fn tick_coil(host: &mut dyn ServerHost, nbt: &mut Compound, items: &mut Vec<Option<ItemStack>>) {
    if items.len() < COIL_SLOTS {
        items.resize(COIL_SLOTS, None);
    }
    let stored = int(nbt, "energy").max(0);
    let recipes = host.recipes_of_type("createaddition:charging");
    let found = (0..COIL_OUTPUT).find_map(|slot| {
        let stack = items[slot].as_ref()?;
        recipes.iter().find_map(|recipe| {
            let charging = parse_charging(&recipe.json)?;
            let ingredient = host.recipe_ingredient(&charging.ingredient).ok()??;
            let result = host.recipe_result(&charging.result).ok()??;
            ingredient
                .test(&*host, stack)
                .then_some((slot, charging, result))
        })
    });
    let Some((slot, charging, result)) = found else {
        if long(nbt, "progress") != 0 {
            set_long(nbt, "progress", 0);
        }
        return;
    };
    let fits = match &items[COIL_OUTPUT] {
        None => true,
        Some(held) => held.item() == result.item() && held.count() + result.count() <= 64,
    };
    if !fits {
        return;
    }
    let (drawn, done) = charge_step(
        long(nbt, "progress"),
        charging.energy,
        charging.max_rate,
        stored,
    );
    set_int(nbt, "energy", stored - drawn);
    if !done {
        set_long(nbt, "progress", long(nbt, "progress") + i64::from(drawn));
        return;
    }
    set_long(nbt, "progress", 0);
    let input = items[slot].take();
    items[slot] = input.and_then(|s| s.shrink(1));
    items[COIL_OUTPUT] = match items[COIL_OUTPUT].take() {
        None => Some(result),
        Some(mut held) => {
            let _ = held.grow(result.count());
            Some(held)
        }
    };
}

/// The game's buffer for a block: what goes in and out, and through which faces.
fn buffer_of(kind: Kind) -> ModEnergyBuffer {
    let all = [true; 6];
    let none = [false; 6];
    let (capacity, max_insert, max_extract, ins, out) = match kind {
        Kind::Motor => (MOTOR_CAPACITY, MOTOR_CAPACITY, 0, all, none),
        Kind::Alternator => (ALTERNATOR_CAPACITY, 0, ALTERNATOR_CAPACITY, none, all),
        Kind::Coil => (COIL_CAPACITY, COIL_TRANSFER, 0, all, none),
        Kind::Accumulator => (
            ACCUMULATOR_CAPACITY,
            ACCUMULATOR_TRANSFER,
            ACCUMULATOR_TRANSFER,
            all,
            all,
        ),
        Kind::Connector => (
            CONNECTOR_CAPACITY,
            CONNECTOR_TRANSFER,
            CONNECTOR_TRANSFER,
            all,
            all,
        ),
        Kind::LargeConnector => (
            LARGE_CONNECTOR_CAPACITY,
            LARGE_CONNECTOR_TRANSFER,
            LARGE_CONNECTOR_TRANSFER,
            all,
            all,
        ),
        Kind::Creative => (CREATIVE_OUTPUT, 0, CREATIVE_OUTPUT, none, all),
    };
    ModEnergyBuffer {
        capacity,
        max_insert,
        max_extract,
        insert_faces: ins,
        extract_faces: out,
    }
}

fn coil_container() -> ModContainer {
    let every: Vec<usize> = (0..COIL_SLOTS).collect();
    ModContainer {
        slots: COIL_SLOTS,
        faces: std::array::from_fn(|_| every.clone()),
        inputs: (0..COIL_OUTPUT).collect(),
        outputs: vec![COIL_OUTPUT],
        menu: Some("minecraft:generic_3x3".into()),
        slot_limit: 0,
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let mut names = Vec::new();
        for (name, kind) in ALL {
            let id = format!("createaddition:{name}");
            let key = r.block_key(&id);
            r.block_energy(&key, buffer_of(kind));
            if kind == Kind::Coil {
                r.block_container(&key, coil_container());
            }
            names.push(id);
        }
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let built = r
            .block_entity_type("createaddition:machine")
            .blocks(&names)
            .build();
        let _ = r.block_entity_part(&built, Machine);
        for name in ["charging", "rolling"] {
            let id = format!("createaddition:{name}");
            r.register_recipe_type(&id);
            r.register_raw_recipe_serializer(&id, &id);
        }
    }
}
