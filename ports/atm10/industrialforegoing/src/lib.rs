//! Industrial Foregoing for Horizon: its core machines, generators, Black Hole storage and
//! addons.
//!
//! Written from Industrial Foregoing's observed behaviour and public description, never from its
//! code (docs/modpacks.md, "Licences"). What the port does, and what a harness against the
//! NeoForge original still has to confirm, is in README.md.
//!
//! Every machine is a block on the vanilla furnace's template (`facing`, `lit`) with a block
//! entity of the one type `industrialforegoing:machine`. Its energy and tanks are the game's own
//! (`r.block_energy`, `r.block_fluids`: the amounts live in the block entity's compound under
//! `energy` and `fluids`), its slots a `ModContainer` that opens a vanilla menu. The slots that
//! are neither inputs nor outputs are addon slots: an addon item (named by its id, or by its `minecraft:custom_data`
//! carrying `industrialforegoing:addon`) changes the machine's numbers ([`rules::Addons`]).
//! A machine works on the block or the area behind it, the side its front faces away from.
use std::sync::OnceLock;

use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockKey,
    BlockPart, Compound, Direction, ENERGY, HostBlockStates, Hooks, Host, HostTransaction, Identifier, ItemKey,
    ItemStack, LevelSide, ModContainer, ModEnergyBuffer, ModFluidTanks, Registrar,
    RegistrationContext, ServerHost, Tag,
};

pub mod rules;
use rules::*;

/// What a block of this mod is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Machine(MachineSpec),
    Generator(GeneratorSpec),
    /// A Black Hole Unit of a tier ([`BLACK_HOLE_TIERS`] index).
    Unit(usize),
    /// A Black Hole Tank of a tier.
    Tank(usize),
}

/// The blocks, in registration order: ids and kinds.
fn blocks() -> Vec<(String, Kind)> {
    let mut v: Vec<(String, Kind)> = MACHINES
        .iter()
        .map(|m| (format!("industrialforegoing:{}", m.name), Kind::Machine(*m)))
        .collect();
    for g in [PITIFUL_GENERATOR, BIOFUEL_GENERATOR] {
        v.push((format!("industrialforegoing:{}", g.name), Kind::Generator(g)));
    }
    for (i, (tier, _, _)) in BLACK_HOLE_TIERS.iter().enumerate() {
        v.push((format!("industrialforegoing:{tier}_black_hole_unit"), Kind::Unit(i)));
        v.push((format!("industrialforegoing:{tier}_black_hole_tank"), Kind::Tank(i)));
    }
    v
}

static KINDS: OnceLock<Vec<(BlockKey, Kind)>> = OnceLock::new();

fn kind_of(block: BlockKey) -> Option<Kind> {
    KINDS.get()?.iter().find(|(k, _)| *k == block).map(|(_, kind)| *kind)
}

fn is_ours(id: &str) -> bool {
    blocks().iter().any(|(b, _)| b == id)
}

/// The container of each kind: slots, inputs, outputs, menu. Slots in neither list are addon
/// slots.
fn container(kind: Kind) -> Option<(usize, Vec<usize>, Vec<usize>, &'static str)> {
    Some(match kind {
        Kind::Machine(m) if m == FLUID_EXTRACTOR => (5, vec![], vec![], "minecraft:hopper"),
        Kind::Machine(m) if m == LATEX_PROCESSING_UNIT => (9, vec![], (0..6).collect(), "minecraft:generic_9x1"),
        Kind::Machine(m) if m == DISSOLUTION_CHAMBER => (18, (0..8).collect(), (8..11).collect(), "minecraft:generic_9x2"),
        Kind::Machine(m) if m == MOB_CRUSHER || m == PLANT_GATHERER => (9, vec![], (0..6).collect(), "minecraft:generic_9x1"),
        Kind::Machine(m) if m == PLANT_SOWER => (9, (0..6).collect(), vec![], "minecraft:generic_9x1"),
        Kind::Machine(_) => return None,
        // The furnace menu: its fuel slot (1) is the generator's; 0 and 2 stay empty.
        Kind::Generator(g) if g == PITIFUL_GENERATOR => (3, vec![1], vec![], "minecraft:furnace"),
        Kind::Generator(_) => return None,
        Kind::Unit(_) => (5, vec![0, 1, 2, 3], vec![4], "minecraft:hopper"),
        Kind::Tank(_) => return None,
    })
}

/// Tank capacities of each kind, with the tanks fills may reach and drains may take.
fn tanks(kind: Kind) -> Option<(Vec<i32>, Vec<usize>, Vec<usize>)> {
    Some(match kind {
        Kind::Machine(m) if m == FLUID_EXTRACTOR => (vec![MACHINE_TANK], vec![], vec![0]),
        Kind::Machine(m) if m == LATEX_PROCESSING_UNIT => (vec![MACHINE_TANK, MACHINE_TANK], vec![0, 1], vec![]),
        Kind::Machine(m) if m == DISSOLUTION_CHAMBER => (vec![MACHINE_TANK, MACHINE_TANK], vec![0], vec![1]),
        Kind::Machine(m) if m == PLANT_GATHERER => (vec![MACHINE_TANK], vec![], vec![0]),
        Kind::Machine(_) => return None,
        Kind::Generator(g) if g == BIOFUEL_GENERATOR => (vec![4_000], vec![0], vec![]),
        Kind::Generator(_) => return None,
        Kind::Unit(_) => return None,
        Kind::Tank(t) => (vec![BLACK_HOLE_TIERS[t].2], vec![0], vec![0]),
    })
}

fn all_faces<T: Clone>(v: Vec<T>) -> [Vec<T>; 6] {
    [v.clone(), v.clone(), v.clone(), v.clone(), v.clone(), v]
}

/// An item key by id, looked up once per call (cheap; the registry is in memory).
fn item(host: &dyn Host, id: &str) -> Option<ItemKey> {
    host.item_key(&Identifier::parse(id).ok()?)
}

/// The custom data of a stack: its own patch's, else its item's default.
fn custom_data(host: &dyn Host, stack: &ItemStack) -> Option<Compound> {
    let own = stack
        .components()
        .and_then(|c| match c.as_nbt().get(&"minecraft:custom_data".into()) {
            Some(Tag::Compound(data)) => Some(data.clone()),
            _ => None,
        });
    own.or_else(|| match host.item_default_component(stack.item(), "minecraft:custom_data") {
        Some(Tag::Compound(data)) => Some(data),
        _ => None,
    })
}

fn addons(host: &dyn Host, items: &[Option<ItemStack>], io: &[usize]) -> Addons {
    let mut a = Addons::default();
    for (i, stack) in items.iter().enumerate() {
        if io.contains(&i) {
            continue;
        }
        let Some(stack) = stack.as_ref() else { continue };
        a.add_item_id(&stack.item().id(host));
        if let Some(data) = custom_data(host, stack) {
            a.add_custom_data(&data);
        }
    }
    a
}

/// Puts `stack` into the first of `slots` with room; returns what did not fit.
fn insert(host: &dyn Host, items: &mut [Option<ItemStack>], slots: &[usize], mut stack: ItemStack) -> Option<ItemStack> {
    let max = host.max_stack_size(stack.item()).unwrap_or(64);
    for &i in slots {
        if let Some(held) = items[i].as_mut() {
            if held.is_stackable_with(&stack) && held.count() < max {
                let moved = (max - held.count()).min(stack.count());
                let _ = held.grow(moved);
                match stack.clone().shrink(moved) {
                    Some(rest) => stack = rest,
                    None => return None,
                }
            }
        }
    }
    for &i in slots {
        if items[i].is_none() {
            let moved = max.min(stack.count());
            let (put, rest) = (stack.clone(), stack.clone().shrink(moved));
            let mut put = put;
            let _ = put.set_count(moved);
            items[i] = Some(put);
            match rest {
                Some(rest) => stack = rest,
                None => return None,
            }
        }
    }
    Some(stack)
}

/// Whether all of `stacks` would fit in `slots`.
fn fits_all(host: &dyn Host, items: &[Option<ItemStack>], slots: &[usize], stacks: &[ItemStack]) -> bool {
    let mut copy = items.to_vec();
    stacks.iter().all(|s| insert(host, &mut copy, slots, s.clone()).is_none())
}

/// A small generator for the machines' chances, saved in the block entity (`rng`), so a
/// machine's draws don't move the level's random.
pub fn next_random(state: &mut i64) -> f64 {
    let mut z = (*state as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
    *state = z as i64;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

struct Machines;

#[mod_api::horizon_part]
impl BlockPart for Machines {}

#[mod_api::horizon_part]
impl BlockEntityPart for Machines {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        is_ours(event.block).then(|| {
            let mut nbt = Compound::new();
            set_int(&mut nbt, "energy", 0);
            nbt
        })
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if is_ours(event.block) && !event.block.ends_with("_black_hole_tank") {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        let Some(kind) = level.block(host, pos).and_then(kind_of) else {
            return;
        };
        let before = (event.entity.nbt.clone(), event.entity.items.clone());
        if let Some((slots, _, _, _)) = container(kind) {
            if event.entity.items.len() < slots {
                event.entity.items.resize(slots, None);
            }
        }
        let facing = level
            .block_property(host, pos, "facing")
            .and_then(|f| facing_normal(&f))
            .unwrap_or([0, 0, -1]);
        let fields = &mut *event.entity;
        let mut cx = Cx { host, level, pos, facing, nbt: &mut fields.nbt, items: &mut fields.items };
        match kind {
            Kind::Machine(m) => machine_tick(&mut cx, m),
            Kind::Generator(g) => generator(&mut cx, g),
            Kind::Unit(t) => black_hole_unit(&mut cx, t),
            Kind::Tank(_) => {}
        }
        if before.0 == event.entity.nbt && before.1 == event.entity.items {
            *event.reply = mod_api::BlockEntityTickReply::Unchanged;
        }
    }
}

/// What a tick works on.
struct Cx<'a> {
    host: &'a mut dyn ServerHost,
    level: mod_api::LevelRef,
    pos: [i32; 3],
    facing: [i32; 3],
    nbt: &'a mut Compound,
    items: &'a mut Vec<Option<ItemStack>>,
}

impl Cx<'_> {
    fn behind(&self) -> [i32; 3] {
        [self.pos[0] - self.facing[0], self.pos[1] - self.facing[1], self.pos[2] - self.facing[2]]
    }
}

fn machine_tick(cx: &mut Cx<'_>, m: MachineSpec) {
    let kind = Kind::Machine(m);
    let (_, inputs, outputs, _) = container(kind).unwrap_or((0, vec![], vec![], ""));
    let io: Vec<usize> = inputs.iter().chain(&outputs).copied().collect();
    let a = addons(cx.host, cx.items, &io);
    let energy = int(cx.nbt, "energy");
    let cost = a.energy_per_operation(m.energy_per_operation);
    let mut progress = int(cx.nbt, "progress");
    let mut tanks_now = tanks(kind).map(|(caps, _, _)| read_tanks(cx.nbt, caps.len())).unwrap_or_default();

    // What the machine would do, and how long its cycle is.
    let (can_work, max_progress) = match m {
        x if x == FLUID_EXTRACTOR => {
            let log = cx.level.block(cx.host, cx.behind());
            let is_log = log.and_then(|b| cx.host.block_in_tag(b, "minecraft:logs")).unwrap_or(false);
            (is_log && tanks_now[0].amount + LATEX_PER_OPERATION <= MACHINE_TANK, m.max_progress)
        }
        x if x == LATEX_PROCESSING_UNIT => {
            let rubber = item(cx.host, "industrialforegoing:tinydryrubber").map(ItemStack::of);
            let ok = tanks_now[0].holds("industrialforegoing:latex")
                && tanks_now[0].amount >= LATEX_PER_RUBBER
                && tanks_now[1].holds("minecraft:water")
                && tanks_now[1].amount >= WATER_PER_RUBBER
                && rubber.is_some_and(|r| fits_all(cx.host, cx.items, &outputs, &[r]));
            (ok, m.max_progress)
        }
        x if x == DISSOLUTION_CHAMBER => {
            let ids = input_ids(cx, &inputs);
            match dissolution_match(&ids, &tanks_now[0], DISSOLUTION_RECIPES) {
                Some(r) => {
                    let out = r.output.and_then(|(id, n)| ItemStack::new(item(cx.host, id)?, n));
                    let item_ok = r.output.is_none() || out.is_some_and(|o| fits_all(cx.host, cx.items, &outputs, &[o]));
                    let fluid_ok = r.output_fluid.is_none_or(|(f, n)| {
                        let mut t = tanks_now[1].clone();
                        t.fill(f, n, MACHINE_TANK) == n
                    });
                    (item_ok && fluid_ok, r.time)
                }
                None => (false, m.max_progress),
            }
        }
        x if x == PLANT_SOWER => (inputs.iter().any(|&i| cx.items[i].is_some()), m.max_progress),
        _ => (true, m.max_progress),
    };

    let step = a.progress_per_tick();
    let done = cycle(&mut progress, max_progress, step, energy, cost, can_work);
    set_int(cx.nbt, "progress", progress);
    if !done {
        return;
    }
    let mut spent = 0;
    for _ in 0..a.operations_per_cycle() {
        if energy - spent < cost {
            break;
        }
        let worked = match m {
            x if x == FLUID_EXTRACTOR => extract_latex(cx, &mut tanks_now),
            x if x == LATEX_PROCESSING_UNIT => process_latex(cx, &mut tanks_now, &outputs),
            x if x == DISSOLUTION_CHAMBER => dissolve(cx, &mut tanks_now, &inputs, &outputs),
            x if x == MOB_CRUSHER => crush(cx, &a, &outputs),
            x if x == PLANT_GATHERER => gather(cx, &a, &mut tanks_now, &outputs),
            x if x == PLANT_SOWER => sow(cx, &a, &inputs),
            _ => false,
        };
        if !worked {
            break;
        }
        spent += cost;
    }
    set_int(cx.nbt, "energy", energy - spent);
    if !tanks_now.is_empty() {
        write_tanks(cx.nbt, &tanks_now);
    }
}

fn input_ids(cx: &Cx<'_>, inputs: &[usize]) -> Vec<Option<String>> {
    inputs
        .iter()
        .map(|&i| cx.items[i].as_ref().map(|s| s.item().id(cx.host)))
        .collect()
}

fn extract_latex(cx: &mut Cx<'_>, tanks_now: &mut [Tank]) -> bool {
    let behind = cx.behind();
    let Some(log) = cx.level.block(cx.host, behind) else {
        return false;
    };
    if !cx.host.block_in_tag(log, "minecraft:logs").unwrap_or(false) {
        return false;
    }
    if tanks_now[0].fill("industrialforegoing:latex", LATEX_PER_OPERATION, MACHINE_TANK) == 0 {
        return false;
    }
    let mut rng = long(cx.nbt, "rng");
    if next_random(&mut rng) < LOG_BREAK_CHANCE {
        cx.level.remove_block(cx.host, behind, false);
    }
    cx.nbt.insert("rng".into(), Tag::Long(rng));
    true
}

fn process_latex(cx: &mut Cx<'_>, tanks_now: &mut [Tank], outputs: &[usize]) -> bool {
    let Some(rubber) = item(cx.host, "industrialforegoing:tinydryrubber").map(ItemStack::of) else {
        return false;
    };
    if tanks_now[0].amount < LATEX_PER_RUBBER || tanks_now[1].amount < WATER_PER_RUBBER || !fits_all(cx.host, cx.items, outputs, &[rubber.clone()]) {
        return false;
    }
    tanks_now[0].drain(LATEX_PER_RUBBER);
    tanks_now[1].drain(WATER_PER_RUBBER);
    insert(cx.host, cx.items, outputs, rubber);
    true
}

fn dissolve(cx: &mut Cx<'_>, tanks_now: &mut [Tank], inputs: &[usize], outputs: &[usize]) -> bool {
    let ids = input_ids(cx, inputs);
    let Some(r) = dissolution_match(&ids, &tanks_now[0], DISSOLUTION_RECIPES).copied() else {
        return false;
    };
    let out = r.output.and_then(|(id, n)| ItemStack::new(item(cx.host, id)?, n));
    if r.output.is_some() && out.is_none() {
        return false;
    }
    if let Some(o) = &out {
        if !fits_all(cx.host, cx.items, outputs, std::slice::from_ref(o)) {
            return false;
        }
    }
    for &i in inputs {
        if let Some(s) = cx.items[i].take() {
            cx.items[i] = s.shrink(1);
        }
    }
    tanks_now[0].drain(r.fluid.1);
    if let Some((f, n)) = r.output_fluid {
        tanks_now[1].fill(f, n, MACHINE_TANK);
    }
    if let Some(o) = out {
        insert(cx.host, cx.items, outputs, o);
    }
    true
}

fn area(cx: &Cx<'_>, a: &Addons, m: MachineSpec) -> ([i32; 3], [i32; 3]) {
    working_area(cx.pos, cx.facing, a.radius(m.radius), m.height, m.y_offset)
}

/// The fake player a crusher kills with, so mobs drop what a player kill drops.
fn fake_player(cx: &mut Cx<'_>) -> Option<mod_api::PlayerRef> {
    let [x, y, z] = cx.pos;
    let uuid = 0x1f00_0000_0000_4000_8000_0000_0000_0000u128
        ^ (u128::from(x as u32) << 64)
        ^ (u128::from(y as u32) << 32)
        ^ u128::from(z as u32);
    cx.host.fake_player(cx.level, uuid, "[IndustrialForegoing]")
}

fn crush(cx: &mut Cx<'_>, a: &Addons, outputs: &[usize]) -> bool {
    let (min, max) = area(cx, a, MOB_CRUSHER);
    let lo = [f64::from(min[0]), f64::from(min[1]), f64::from(min[2])];
    let hi = [f64::from(max[0] + 1), f64::from(max[1] + 1), f64::from(max[2] + 1)];
    let Some(entities) = cx.level.entities_in(cx.host, lo, hi) else {
        return false;
    };
    let victim = entities.iter().copied().find(|e| {
        let Some(kind) = e.kind(cx.host).map(|k| k.id(cx.host)) else {
            return false;
        };
        !crusher_spares(&kind)
            && cx.host.living(*e).is_some_and(|l| l.alive(cx.host).unwrap_or(false))
    });
    let mut worked = false;
    if let Some(victim) = victim {
        let killer = fake_player(cx).map(|p| p.entity());
        worked = victim
            .hurt(cx.host, "minecraft:player_attack", f32::MAX, killer, killer)
            .unwrap_or(false);
    }
    // Pick up the drops lying in the area.
    let entities = cx.level.entities_in(cx.host, lo, hi).unwrap_or_default();
    for e in entities {
        let Some(drop) = cx.host.item_entity(e) else {
            continue;
        };
        let Some(stack) = drop.stack(cx.host) else {
            continue;
        };
        match insert(cx.host, cx.items, outputs, stack.clone()) {
            None => {
                e.discard(cx.host);
                worked = true;
            }
            Some(rest) if rest.count() < stack.count() => {
                let _ = drop.set_stack(cx.host, rest);
                worked = true;
            }
            Some(_) => {}
        }
    }
    worked
}

/// What the gatherer does with a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Harvest {
    /// A ripe crop: break it and plant it again.
    Replant,
    /// A melon or pumpkin: break it.
    Break,
    /// A sugar cane, cactus or bamboo stalk on more of itself: break it, leaving the bottom.
    Stalk,
}

/// The gatherer's rule for a block: `id`, its age and its highest age if it has one, the id of
/// the block below.
pub fn harvest_rule(id: &str, is_crop: bool, age: Option<(i32, i32)>, below: Option<&str>) -> Option<Harvest> {
    match id {
        "minecraft:melon" | "minecraft:pumpkin" => Some(Harvest::Break),
        "minecraft:sugar_cane" | "minecraft:cactus" | "minecraft:bamboo" => (below == Some(id)).then_some(Harvest::Stalk),
        _ if is_crop || id == "minecraft:nether_wart" => match age {
            Some((age, max)) if age >= max => Some(Harvest::Replant),
            _ => None,
        },
        _ => None,
    }
}

fn gather(cx: &mut Cx<'_>, a: &Addons, tanks_now: &mut [Tank], outputs: &[usize]) -> bool {
    let (min, max) = area(cx, a, PLANT_GATHERER);
    let n = area_size(min, max);
    let start = int(cx.nbt, "cursor").max(0) as u32;
    // Look at most at the whole area once per operation for something to harvest.
    for k in 0..n {
        let index = (start + k) % n;
        let pos = area_position(min, max, index);
        let Some(block) = cx.level.block(cx.host, pos) else {
            continue;
        };
        let id = block.id(cx.host);
        let is_crop = cx.host.block_in_tag(block, "minecraft:crops").unwrap_or(false);
        let age = cx.level.block_property(cx.host, pos, "age").and_then(|v| v.parse::<i32>().ok()).map(|age| {
            let max = cx
                .host
                .block_states()
                .definition(block)
                .and_then(|d| d.into_iter().find(|p| p.name == "age"))
                .and_then(|p| p.values.iter().filter_map(|v| v.parse::<i32>().ok()).max())
                .unwrap_or(age);
            (age, max)
        });
        let below = cx.level.block(cx.host, [pos[0], pos[1] - 1, pos[2]]).map(|b| b.id(cx.host));
        let Some(rule) = harvest_rule(&id, is_crop, age, below.as_deref()) else {
            continue;
        };
        let Ok(Some(mut drops)) = cx.level.destroy_block(cx.host, pos, true, None, true) else {
            continue;
        };
        if rule == Harvest::Replant {
            if let Some(seed) = cx.host.block_item(block) {
                if let Some(i) = drops.iter().position(|s| s.item() == seed) {
                    let s = drops.remove(i);
                    if let Some(rest) = s.shrink(1) {
                        drops.push(rest);
                    }
                }
            }
            if let Some(state) = cx.host.block_state_default(block) {
                let flags = mod_api::BlockUpdateFlags::NEIGHBORS | mod_api::BlockUpdateFlags::CLIENTS;
                let _ = cx.level.set_block(cx.host, pos, state, flags);
            }
        }
        for d in drops {
            if let Some(rest) = insert(cx.host, cx.items, outputs, d) {
                let _ = cx.level.spawn_item(cx.host, f64::from(cx.pos[0]) + 0.5, f64::from(cx.pos[1]) + 1.0, f64::from(cx.pos[2]) + 0.5, rest, 10);
            }
        }
        tanks_now[0].fill("industrialforegoing:sludge", SLUDGE_PER_HARVEST, MACHINE_TANK);
        set_int(cx.nbt, "cursor", ((index + 1) % n) as i32);
        return true;
    }
    false
}

/// The soil a planted block needs: `plant` the block's id, whether it is a crop and whether a
/// sapling; `soil` the block below and whether it is in `minecraft:dirt`.
pub fn soil_takes(plant: &str, is_crop: bool, is_sapling: bool, soil: &str, soil_is_dirt: bool) -> bool {
    if plant == "minecraft:nether_wart" {
        soil == "minecraft:soul_sand"
    } else if is_crop {
        soil == "minecraft:farmland"
    } else if is_sapling {
        soil_is_dirt
    } else {
        false
    }
}

fn sow(cx: &mut Cx<'_>, a: &Addons, inputs: &[usize]) -> bool {
    let (min, max) = area(cx, a, PLANT_SOWER);
    let n = area_size(min, max);
    let start = int(cx.nbt, "cursor").max(0) as u32;
    for k in 0..n {
        let index = (start + k) % n;
        let pos = area_position(min, max, index);
        if cx.level.block(cx.host, pos).map(|b| b.id(cx.host)).as_deref() != Some("minecraft:air") {
            continue;
        }
        let Some(soil) = cx.level.block(cx.host, [pos[0], pos[1] - 1, pos[2]]) else {
            continue;
        };
        let soil_id = soil.id(cx.host);
        let soil_is_dirt = cx.host.block_in_tag(soil, "minecraft:dirt").unwrap_or(false);
        for &slot in inputs {
            let Some(stack) = cx.items[slot].clone() else {
                continue;
            };
            let Some(plant) = cx.host.item_block(stack.item()) else {
                continue;
            };
            let plant_id = plant.id(cx.host);
            let is_crop = cx.host.block_in_tag(plant, "minecraft:crops").unwrap_or(false);
            let is_sapling = cx.host.block_in_tag(plant, "minecraft:saplings").unwrap_or(false);
            if !soil_takes(&plant_id, is_crop, is_sapling, &soil_id, soil_is_dirt) {
                continue;
            }
            let Some(state) = cx.host.block_state_default(plant) else {
                continue;
            };
            let flags = mod_api::BlockUpdateFlags::NEIGHBORS | mod_api::BlockUpdateFlags::CLIENTS;
            if cx.level.set_block(cx.host, pos, state, flags) != Ok(true) {
                continue;
            }
            cx.items[slot] = stack.shrink(1);
            set_int(cx.nbt, "cursor", ((index + 1) % n) as i32);
            return true;
        }
    }
    false
}

fn generator(cx: &mut Cx<'_>, g: GeneratorSpec) {
    let mut energy = int(cx.nbt, "energy");
    let mut burn = int(cx.nbt, "burn");
    let made = if g == PITIFUL_GENERATOR {
        let fuel = cx.items.get(1).cloned().flatten();
        let next = fuel.as_ref().and_then(|f| cx.host.item_burn_time(f.item()));
        let (made, took) = generator_tick(&mut burn, energy, &g, next);
        if took {
            if let Some(f) = fuel {
                let remainder = cx.host.item_crafting_remainder(&f);
                cx.items[1] = f.shrink(1).or(remainder);
            }
            set_int(cx.nbt, "burn_total", burn + 1);
        }
        made
    } else {
        let mut t = read_tanks(cx.nbt, 1);
        let has = t[0].holds("industrialforegoing:biofuel") && t[0].amount >= BIOFUEL_PER_TICK;
        let (made, took) = generator_tick(&mut burn, energy, &g, has.then_some(1));
        if took {
            t[0].drain(BIOFUEL_PER_TICK);
            write_tanks(cx.nbt, &t);
        }
        made
    };
    energy += made;
    set_int(cx.nbt, "burn", burn.max(0));
    // Push to every neighbour that takes energy.
    let mut left = g.push.min(energy);
    for dir in Direction::ALL {
        if left <= 0 {
            break;
        }
        let n = dir.normal();
        let at = [cx.pos[0] + n[0], cx.pos[1] + n[1], cx.pos[2] + n[2]];
        let Some(cap) = cx.level.capability(cx.host, &ENERGY, at, Some(dir.opposite())) else {
            continue;
        };
        let moved = cx
            .host
            .transaction(|host, tx| Ok::<u32, ()>(cap.insert(host, tx, left as u32)))
            .unwrap_or(0) as i32;
        left -= moved;
        energy -= moved;
    }
    set_int(cx.nbt, "energy", energy);
}

/// A Black Hole Unit: its input slots empty into its store, its output slot is kept full from
/// it. The store is the item (`stored`, the stack's saved form with count 1) and `count`.
fn black_hole_unit(cx: &mut Cx<'_>, tier: usize) {
    let capacity = BLACK_HOLE_TIERS[tier].1;
    let mut stored = match cx.nbt.get(&"stored".into()) {
        Some(Tag::Compound(c)) => ItemStack::from_nbt(cx.host, c),
        _ => None,
    };
    let mut count = long(cx.nbt, "count");
    if stored.is_none() {
        count = 0;
    }
    // Put the output slot's items back first, so it is refilled from one count.
    if let Some(out) = cx.items[4].take() {
        match &stored {
            Some(s) if s.is_stackable_with(&out) => count += i64::from(out.count()),
            None => {
                count = i64::from(out.count());
                let mut one = out.clone();
                let _ = one.set_count(1);
                stored = Some(one);
            }
            Some(_) => cx.items[4] = Some(out),
        }
    }
    for slot in 0..4 {
        let Some(input) = cx.items[slot].clone() else {
            continue;
        };
        if let Some(s) = &stored {
            if !s.is_stackable_with(&input) {
                continue;
            }
        } else {
            let mut one = input.clone();
            let _ = one.set_count(1);
            stored = Some(one);
            count = 0;
        }
        let took = black_hole_accepts(count, input.count(), capacity);
        count += i64::from(took);
        cx.items[slot] = input.shrink(took);
    }
    if cx.items[4].is_none() {
        if let Some(s) = &stored {
            let max = cx.host.max_stack_size(s.item()).unwrap_or(64);
            let n = (count.min(i64::from(max))) as u32;
            if n > 0 {
                let mut out = s.clone();
                let _ = out.set_count(n);
                cx.items[4] = Some(out);
                count -= i64::from(n);
            }
        }
    }
    if count == 0 && cx.items[4].is_none() {
        stored = None;
    }
    match stored.as_ref().and_then(|s| s.to_nbt(cx.host)) {
        Some(c) => {
            cx.nbt.insert("stored".into(), Tag::Compound(c));
        }
        None => {
            cx.nbt.remove(&"stored".into());
        }
    }
    cx.nbt.insert("count".into(), Tag::Long(count));
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let all = blocks();
        let mut kinds = Vec::new();
        let mut names = Vec::new();
        for (id, kind) in &all {
            let key = r.block_key(id);
            if let Some((slots, inputs, outputs, menu)) = container(*kind) {
                let mut faces = inputs.clone();
                faces.extend(&outputs);
                r.block_container(
                    &key,
                    ModContainer { slots, faces: all_faces(faces), inputs, outputs, menu: Some(menu.into()) },
                );
            }
            if let Some((caps, fills, drains)) = tanks(*kind) {
                let mut t = ModFluidTanks::default();
                let mut faces = fills.clone();
                for d in &drains {
                    if !faces.contains(d) {
                        faces.push(*d);
                    }
                }
                t.tanks = caps;
                t.faces = all_faces(faces);
                t.inputs = fills;
                t.outputs = drains;
                r.block_fluids(&key, t);
            }
            let energy = match kind {
                Kind::Machine(m) => Some(ModEnergyBuffer { capacity: m.energy_capacity, max_insert: m.energy_capacity, max_extract: 0, insert_faces: [true; 6], extract_faces: [false; 6] }),
                Kind::Generator(g) => Some(ModEnergyBuffer { capacity: g.capacity, max_insert: 0, max_extract: g.push, insert_faces: [false; 6], extract_faces: [true; 6] }),
                _ => None,
            };
            if let Some(e) = energy {
                r.block_energy(&key, e);
            }
            kinds.push((key, *kind));
            names.push(id.as_str());
        }
        let _ = KINDS.set(kinds);
        let kind = r.block_entity_type("machine").blocks(&names).build();
        let _ = r.block_entity_part(&kind, Machines);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gatherer_takes_ripe_crops_whole_gourds_and_stalks_above_their_base() {
        assert_eq!(harvest_rule("minecraft:wheat", true, Some((7, 7)), Some("minecraft:farmland")), Some(Harvest::Replant));
        assert_eq!(harvest_rule("minecraft:wheat", true, Some((6, 7)), Some("minecraft:farmland")), None);
        assert_eq!(harvest_rule("minecraft:nether_wart", false, Some((3, 3)), None), Some(Harvest::Replant));
        assert_eq!(harvest_rule("minecraft:melon", false, None, None), Some(Harvest::Break));
        assert_eq!(harvest_rule("minecraft:sugar_cane", false, None, Some("minecraft:sugar_cane")), Some(Harvest::Stalk));
        assert_eq!(harvest_rule("minecraft:sugar_cane", false, None, Some("minecraft:sand")), None);
        assert_eq!(harvest_rule("minecraft:stone", false, None, None), None);
    }

    #[test]
    fn the_sower_plants_each_seed_only_on_its_soil() {
        assert!(soil_takes("minecraft:wheat", true, false, "minecraft:farmland", false));
        assert!(!soil_takes("minecraft:wheat", true, false, "minecraft:dirt", true));
        assert!(soil_takes("minecraft:nether_wart", false, false, "minecraft:soul_sand", false));
        assert!(soil_takes("minecraft:oak_sapling", false, true, "minecraft:grass_block", true));
        assert!(!soil_takes("minecraft:oak_sapling", false, true, "minecraft:stone", false));
        assert!(!soil_takes("minecraft:torch", false, false, "minecraft:farmland", false));
    }

    #[test]
    fn the_machines_random_is_uniform_enough_for_a_one_percent_chance() {
        let mut s = 12345;
        let hits = (0..100_000).filter(|_| next_random(&mut s) < LOG_BREAK_CHANCE).count();
        assert!((800..1200).contains(&hits), "{hits}");
    }
}
