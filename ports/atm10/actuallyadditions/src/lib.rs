//! Actually Additions for Horizon: the coal generator, the powered furnace and the crusher.
//!
//! Written from Actually Additions' observed behaviour and public description, never from its
//! code (docs/modpacks.md, "Licences"). README.md lists what is ported and which numbers a check
//! against the NeoForge original still has to confirm.
//!
//! The three blocks stand on the vanilla furnace's template (its `facing` and `lit`). Energy is the
//! game's `ModEnergyBuffer` (saved `energy`), so cables and other mods reach it through `ENERGY`.
//! Every machine opens the vanilla hopper menu, whose five slots are laid out per machine in
//! [`Kind::layout`].
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Compound, Direction, Hooks, Host, HostTransaction,
    Identifier, ItemStack, LevelRef, LevelSide, ModContainer, ModEnergyBuffer, RecipeInput,
    Registrar, RegistrationContext, ServerHost, Tag, ENERGY,
};

pub mod rules;
use rules::*;

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

/// Whether `result` fits on top of what a result slot holds.
fn fits(host: &dyn Host, held: &Option<ItemStack>, result: &ItemStack) -> bool {
    match held {
        None => true,
        Some(h) => {
            let max = host.max_stack_size(h.item()).unwrap_or(64);
            h.is_stackable_with(result) && h.count() + result.count() <= max
        }
    }
}

fn put(slot: &mut Option<ItemStack>, stack: ItemStack) {
    match slot {
        None => *slot = Some(stack),
        Some(held) => {
            let _ = held.grow(stack.count());
        }
    }
}

/// Offers `amount` FE to every neighbour that takes energy: how much they took.
fn push_energy(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], amount: u32) -> u32 {
    let mut left = amount;
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
    }
    amount - left
}

struct Machine;

#[mod_api::horizon_part]
impl BlockPart for Machine {}

#[mod_api::horizon_part]
impl BlockEntityPart for Machine {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        Kind::of_block(event.block)?;
        let mut nbt = Compound::new();
        set_int(&mut nbt, "energy", 0);
        Some(nbt)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if Kind::of_block(event.block).is_some() {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        let Some(kind) = level
            .block(&*host, pos)
            .and_then(|k| Kind::of_block(&k.id(&*host)))
        else {
            return;
        };
        let before_nbt = event.entity.nbt.clone();
        let before_items = event.entity.items.clone();
        if event.entity.items.len() < SLOTS {
            event.entity.items.resize(SLOTS, None);
        }
        let mut energy = int(&event.entity.nbt, "energy").max(0);
        let working;
        match kind {
            Kind::CoalGenerator => {
                let mut burn = Burn { left: int(&event.entity.nbt, "burn"), total: int(&event.entity.nbt, "burn_total") };
                let fuel_burn = event.entity.items[0]
                    .as_ref()
                    .and_then(|f| host.item_burn_time(f.item()))
                    .filter(|b| *b > 0);
                let step = burn.tick(energy, COAL_GENERATOR.capacity, COAL_GENERATOR.per_tick, fuel_burn);
                if step.consume_fuel {
                    if let Some(f) = event.entity.items[0].take() {
                        let remainder = host.item_crafting_remainder(&f);
                        event.entity.items[0] = f.shrink(1).or(remainder);
                    }
                }
                energy += step.energy;
                working = step.energy > 0;
                set_int(&mut event.entity.nbt, "burn", burn.left);
                set_int(&mut event.entity.nbt, "burn_total", burn.total);
                let given = push_energy(host, level, pos, energy.min(COAL_GENERATOR.per_tick * 4) as u32);
                energy -= given as i32;
            }
            Kind::PoweredFurnace | Kind::Crusher => {
                let spec = kind.machine();
                let (inputs, outputs) = kind.layout();
                let mut progress = int(&event.entity.nbt, "progress");
                // Pick the first input slot that has a result that fits.
                let mut job: Option<(usize, usize, ItemStack)> = None;
                for (i, out) in inputs.iter().zip(outputs.iter()) {
                    let Some(stack) = event.entity.items[*i].clone() else { continue };
                    let result = match kind {
                        Kind::PoweredFurnace => Identifier::parse("minecraft:smelting")
                            .ok()
                            .and_then(|k| {
                                level.recipe_for(&*host, &k, &RecipeInput::Single(stack.clone())).ok().flatten()
                            })
                            .map(|m| m.output),
                        _ => {
                            let id = stack.item().id(&*host);
                            crush(&id).and_then(|c| {
                                let key = host.item_key(&Identifier::parse(c.primary.0).ok()?)?;
                                ItemStack::new(key, c.primary.1)
                            })
                        }
                    };
                    if let Some(result) = result.filter(|r| fits(&*host, &event.entity.items[*out], r)) {
                        job = Some((*i, *out, result));
                        break;
                    }
                }
                let step = work_step(&mut progress, energy, spec, job.is_some());
                energy -= step.energy_used;
                working = step.energy_used > 0;
                if step.done {
                    if let Some((i, out, result)) = job {
                        let input = event.entity.items[i].take();
                        let id = input.as_ref().map(|s| s.item().id(&*host)).unwrap_or_default();
                        event.entity.items[i] = input.and_then(|s| s.shrink(1));
                        put(&mut event.entity.items[out], result);
                        if kind == Kind::Crusher {
                            // the secondary output, 20 % by default, on a roll of the game time
                            let time = level.game_time(&*host).unwrap_or(0);
                            if let Some(c) = crush(&id) {
                                if let Some((sid, n, chance)) = c.secondary {
                                    if roll(time, pos) < chance {
                                        let sec = Identifier::parse(sid)
                                            .ok()
                                            .and_then(|i| host.item_key(&i))
                                            .and_then(|k| ItemStack::new(k, n));
                                        if let Some(sec) = sec {
                                            if fits(&*host, &event.entity.items[SECONDARY], &sec) {
                                                put(&mut event.entity.items[SECONDARY], sec);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                set_int(&mut event.entity.nbt, "progress", progress);
            }
        }
        set_int(&mut event.entity.nbt, "energy", energy);
        let lit = i32::from(working);
        if int(&event.entity.nbt, "lit") != lit {
            set_int(&mut event.entity.nbt, "lit", lit);
            host.set_block_property(pos, "lit", if working { "true" } else { "false" });
        }
        if event.entity.nbt == before_nbt && event.entity.items == before_items {
            *event.reply = BlockEntityTickReply::Unchanged;
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for kind in Kind::ALL {
            let id = kind.block_id();
            let key = r.block_key(&id);
            let (cap, insert, extract) = kind.energy();
            let (all, none) = ([true; 6], [false; 6]);
            r.block_energy(
                &key,
                ModEnergyBuffer {
                    capacity: cap,
                    max_insert: insert,
                    max_extract: extract,
                    insert_faces: if insert > 0 { all } else { none },
                    extract_faces: if extract > 0 { all } else { none },
                },
            );
            let (inputs, outputs) = kind.layout();
            let mut ins: Vec<usize> = inputs.to_vec();
            let mut outs: Vec<usize> = outputs.to_vec();
            if kind == Kind::Crusher {
                outs.push(SECONDARY);
            }
            let every: Vec<usize> = ins.iter().chain(outs.iter()).copied().collect();
            if kind == Kind::CoalGenerator {
                ins = vec![0];
                outs = vec![];
            }
            r.block_container(
                &key,
                ModContainer {
                    slots: SLOTS,
                    faces: std::array::from_fn(|_| if kind == Kind::CoalGenerator { vec![0] } else { every.clone() }),
                    inputs: ins,
                    outputs: outs,
                    menu: Some("minecraft:hopper".into()),
                },
            );
        }
        let names: Vec<String> = Kind::ALL.iter().map(|k| k.block_id()).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let kind = r.block_entity_type("machine").blocks(&refs).build();
        let _ = r.block_entity_part(&kind, Machine);
    }
}
