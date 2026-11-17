//! Hostile Neural Networks for Horizon: data models that learn from kills, the simulation
//! chamber and the loot fabricator.
//!
//! Written from Hostile Neural Networks' observed behaviour and public description, never from
//! its code (docs/modpacks.md, "Licences"). README.md lists what is ported and what a check
//! against the NeoForge original still has to confirm.
//!
//! A data model (`data_model_<mob>`) keeps its data in `minecraft:custom_data` (`hostile_data`).
//! A player carrying one gives it a point of data whenever they kill that mob in melee
//! (`mob_killed`). The simulation chamber turns a model of tier basic or better plus a polymer clay
//! into a prediction (`prediction_<mob>`); the loot fabricator turns a prediction into that mob's
//! loot. Both draw energy from the game's `ModEnergyBuffer`.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Compound, Hooks, Host, HostTransaction, Identifier,
    ItemPlace, ItemResource, ItemStack, LevelSide, ModContainer, ModEnergyBuffer, MobKilledEvent,
    Registrar, RegistrationContext, ServerHost, Tag,
};

pub mod rules;
use rules::*;

const SIM: &str = "hostilenetworks:sim_chamber";
const FAB: &str = "hostilenetworks:loot_fabricator";

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

/// The data a model stack holds.
fn data_of(stack: &ItemStack) -> u32 {
    let Some(components) = stack.components() else { return 0 };
    match components.as_nbt().get(&"minecraft:custom_data".into()) {
        Some(Tag::Compound(c)) => int(c, "hostile_data").max(0) as u32,
        _ => 0,
    }
}

fn with_data(stack: &ItemStack, data: u32) -> Option<ItemStack> {
    let mut custom = Compound::new();
    custom.insert("hostile_data".into(), Tag::Int(data as i32));
    let mut patch = Compound::new();
    patch.insert("minecraft:custom_data".into(), Tag::Compound(custom));
    let mut out = stack.clone();
    out.apply_components(&patch).ok()?;
    Some(out)
}

fn item(host: &dyn Host, id: &str, count: u32) -> Option<ItemStack> {
    ItemStack::new(host.item_key(&Identifier::parse(id).ok()?)?, count)
}

fn fits(host: &dyn Host, held: &Option<ItemStack>, add: &ItemStack) -> bool {
    match held {
        None => true,
        Some(h) => h.is_stackable_with(add) && h.count() + add.count() <= host.max_stack_size(h.item()).unwrap_or(64),
    }
}

fn put(slot: &mut Option<ItemStack>, stack: ItemStack) {
    match slot {
        None => *slot = Some(stack),
        Some(h) => {
            let _ = h.grow(stack.count());
        }
    }
}

struct Machines;

#[mod_api::horizon_part]
impl BlockPart for Machines {}

#[mod_api::horizon_part]
impl BlockEntityPart for Machines {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == SIM || event.block == FAB).then(|| {
            let mut nbt = Compound::new();
            nbt.insert("energy".into(), Tag::Int(0));
            nbt
        })
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == SIM || event.block == FAB {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let Some(level) = host.level(host.dimension()) else { return };
        let Some(id) = level.block(&*host, pos).map(|k| k.id(&*host)) else { return };
        let sim = id == SIM;
        if !sim && id != FAB {
            return;
        }
        let mut nbt = event.entity.nbt.clone();
        let mut items = event.entity.items.clone();
        if items.len() < 5 {
            items.resize(5, None);
        }
        let (spec, out) = if sim { (SIM_CHAMBER, 2) } else { (LOOT_FABRICATOR, 1) };
        // what a run would make, and what it takes
        let job: Option<(Vec<usize>, ItemStack)> = if sim {
            let model = items[0].as_ref().and_then(|m| {
                let mob = mob_of_item(&m.item().id(&*host), "data_model_")?;
                Tier::of(data_of(m)).simulates().then_some(mob)
            });
            let has_clay = items[1].as_ref().is_some_and(|c| c.item().id(&*host) == "hostilenetworks:polymer_clay");
            model
                .filter(|_| has_clay)
                .and_then(|mob| item(&*host, &format!("hostilenetworks:prediction_{}", mob.name), 1))
                .map(|p| (vec![1], p))
        } else {
            items[0]
                .as_ref()
                .and_then(|p| mob_of_item(&p.item().id(&*host), "prediction_"))
                .and_then(|mob| item(&*host, mob.loot.0, mob.loot.1))
                .map(|l| (vec![0], l))
        };
        let job = job.filter(|(_, result)| fits(&*host, &items[out], result));
        let mut progress = int(&nbt, "progress");
        let mut energy = int(&nbt, "energy");
        let (used, done) = step(&mut progress, energy, spec, job.is_some());
        energy -= used;
        if done {
            if let Some((takes, result)) = job {
                for t in takes {
                    items[t] = items[t].take().and_then(|s| s.shrink(1));
                }
                put(&mut items[out], result);
            }
        }
        nbt.insert("progress".into(), Tag::Int(progress));
        nbt.insert("energy".into(), Tag::Int(energy));
        let lit = i32::from(used > 0);
        if int(&nbt, "lit") != lit {
            nbt.insert("lit".into(), Tag::Int(lit));
            host.set_block_property(pos, "lit", if lit == 1 { "true" } else { "false" });
        }
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
        for (id, inputs, outputs) in [(SIM, vec![0, 1], vec![2]), (FAB, vec![0], vec![1])] {
            let key = r.block_key(id);
            r.block_energy(
                &key,
                ModEnergyBuffer { capacity: BUFFER, max_insert: INSERT, max_extract: 0, insert_faces: [true; 6], extract_faces: [false; 6] },
            );
            let every: Vec<usize> = inputs.iter().chain(&outputs).copied().collect();
            r.block_container(
                &key,
                ModContainer {
                    slots: 5,
                    faces: std::array::from_fn(|_| every.clone()),
                    inputs,
                    outputs,
                    menu: Some("minecraft:hopper".into()),
                    slot_limit: 0,
                },
            );
        }
        let kind = r.block_entity_type("machine").blocks(&[SIM, FAB]).build();
        let _ = r.block_entity_part(&kind, Machines);
    }

    /// A player's melee kill gives a data point to every data model of that mob they carry.
    fn mob_killed(&self, host: &mut dyn ServerHost, event: &mut MobKilledEvent<'_>) {
        let Some(mob) = mob_of_entity(event.kind) else { return };
        let Some(player) = host.player(event.player) else { return };
        let model = format!("hostilenetworks:data_model_{}", mob.name);
        for (slot, stack) in player.inventory_slots(&*host).unwrap_or_default() {
            if stack.item().id(&*host) != model {
                continue;
            }
            let Some(learned) = with_data(&stack, learn(data_of(&stack), 1)) else { continue };
            let place = ItemPlace::PlayerSlot { player: event.player, slot: slot as i32 };
            let _ = host.transaction(|host, tx| {
                place.exchange(host, tx, &ItemResource::of(&learned), 1);
                Ok::<(), ()>(())
            });
        }
    }
}
