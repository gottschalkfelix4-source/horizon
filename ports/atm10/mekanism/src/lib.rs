//! Mekanism for Horizon: ores and the metal chain, energy cubes, universal cables, the first
//! machines (enrichment chamber, crusher, energized smelter) and the `mekanism:chemicals`
//! capability for gases.
//!
//! Written from Mekanism's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and which numbers a check against the
//! NeoForge original still has to confirm, is in README.md.
//!
//! Energy lives in each block entity's saved `energy` (the game's `ModEnergyBuffer`), so other
//! mods' machines reach it through `ENERGY`. A cube pushes into adjacent machines; a network of
//! universal cables, run by its lowest cable, carries energy from the cubes it touches to the
//! machines it touches.
use std::collections::{BTreeSet, VecDeque};

use mod_api::{
    CapabilityKey, FLUIDS, ITEMS, ResourceHandler, move_resources,
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Compound, Direction, ENERGY, Hooks, Host, Identifier,
    HostTransaction, ItemStack, LevelRef, LevelSide, ModContainer, ModEnergyBuffer, ModFluidTanks, RecipeInput, Registrar,
    RegistrationContext, ServerHost, Tag,
};

pub mod chemical;
use chemical::{BlockTanks, CHEMICALS, Tank, TankSpec};
pub mod matrix;
pub mod mb;
pub mod multiblock;
pub mod rules;
pub mod steam;
pub mod tank;
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

fn kind_at(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<Kind> {
    parse_block(&level.block(host, pos)?.id(host))
}

/// Moves up to `budget` FE from the energy of `from` to the energy of `to`, in one transaction:
/// how much moved (at most what the source can hand over and the sink take).
fn route(
    host: &mut dyn ServerHost,
    level: LevelRef,
    from: ([i32; 3], Direction),
    to: ([i32; 3], Direction),
    budget: u32,
) -> u32 {
    let (Some(src), Some(dst)) = (
        level.capability(host, &ENERGY, from.0, Some(from.1)),
        level.capability(host, &ENERGY, to.0, Some(to.1)),
    ) else {
        return 0;
    };
    // What the source can give this tick (its holdings, its rate), found in a transaction that
    // is thrown away: asking for more than it can hand over would abort the move.
    let held = host
        .transaction(|host, tx| {
            let have = src.amount(host).min(budget);
            let n = src.extract(host, tx, have);
            Err::<(), u32>(n)
        })
        .err()
        .unwrap_or(0);
    if held == 0 {
        return 0;
    }
    host.transaction(|host, tx| {
        let taken = dst.insert(host, tx, held);
        if taken == 0 {
            return Ok(0);
        }
        if src.extract(host, tx, taken) < taken {
            return Err(());
        }
        Ok::<u32, ()>(taken)
    })
    .unwrap_or(0)
}

/// The cables joined to `start` (up to 4096), sorted, the lowest first.
fn network(host: &dyn Host, level: LevelRef, start: [i32; 3]) -> Vec<[i32; 3]> {
    network_of(host, level, start, |k| matches!(k, Kind::Cable(_)))
}

/// The blocks `same` accepts, joined to `start` (up to 4096), sorted, the lowest first.
fn network_of(
    host: &dyn Host,
    level: LevelRef,
    start: [i32; 3],
    same: impl Fn(Kind) -> bool,
) -> Vec<[i32; 3]> {
    let mut seen = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(pos) = queue.pop_front() {
        if seen.len() >= 4096 {
            break;
        }
        for dir in Direction::ALL {
            let n = offset(pos, dir);
            if !seen.contains(&n) && kind_at(host, level, n).is_some_and(&same) {
                seen.insert(n);
                queue.push_back(n);
            }
        }
    }
    seen.into_iter().collect()
}

/// One tick of the cable network `pos` belongs to, run only by its lowest cable.
fn tick_cable(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], tier: Tier) {
    let cables = network(&*host, level, pos);
    if cables.first() != Some(&pos) {
        return;
    }
    let mut rate = tier.cable_rate();
    let (mut sources, mut sinks) = (Vec::new(), Vec::new());
    let (mut cubes, mut foreign) = (Vec::new(), Vec::new());
    for &c in &cables {
        if let Some(Kind::Cable(t)) = kind_at(&*host, level, c) {
            rate = rate.min(t.cable_rate());
        }
        for dir in Direction::ALL {
            let n = offset(c, dir);
            match kind_at(&*host, level, n) {
                Some(Kind::Cable(_) | Kind::Pipe(..)) => {}
                Some(Kind::Cube(_)) => {
                    sources.push((n, dir.opposite()));
                    cubes.push((n, dir.opposite()));
                }
                Some(Kind::Machine(_) | Kind::Factory(..)) => sinks.push((n, dir.opposite())),
                // another mod's block with energy (a generator, a cell): it gives and takes
                None => {
                    let side = dir.opposite();
                    if level.capability(host, &ENERGY, n, Some(side)).is_some() {
                        sources.push((n, side));
                        foreign.push((n, side));
                    }
                }
            }
        }
    }
    let cubes_in = cubes.clone();
    // machines first, then cubes and the foreign blocks: they take what is left
    sinks.extend(cubes);
    sinks.extend(foreign);
    let mut budget = rate.max(0) as u32;
    for &sink in &sinks {
        for &source in &sources {
            if budget == 0 {
                return;
            }
            if source.0 == sink.0 || (cubes_in.contains(&source) && cubes_in.contains(&sink)) {
                continue;
            }
            budget -= route(host, level, source, sink, budget);
        }
    }
}

/// Moves up to `budget` of `R` between the endpoints of a pipe network: out of each endpoint,
/// into the others, in one transaction per pair.
fn move_network<R: 'static>(
    host: &mut dyn ServerHost,
    level: LevelRef,
    key: &CapabilityKey<dyn ResourceHandler<R>>,
    ends: &[([i32; 3], Direction)],
    mut budget: u32,
) {
    for (i, &from) in ends.iter().enumerate() {
        for (j, &to) in ends.iter().enumerate() {
            if budget == 0 {
                return;
            }
            if i == j {
                continue;
            }
            let (Some(src), Some(dst)) = (
                level.capability(host, key, from.0, Some(from.1)),
                level.capability(host, key, to.0, Some(to.1)),
            ) else {
                continue;
            };
            let moved = host
                .transaction(|host, tx| {
                    move_resources(host, &src, &dst, |_| true, budget, tx).map_err(|_| ())
                })
                .unwrap_or(0);
            budget -= moved.min(budget);
        }
    }
}

/// One tick of the pipe network `pos` belongs to, run only by its lowest pipe: what the blocks
/// touching it give is moved to the others, at the lowest tier's rate.
fn tick_pipe(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], family: Family) {
    let pipes = network_of(&*host, level, pos, |k| matches!(k, Kind::Pipe(f, _) if f == family));
    if pipes.first() != Some(&pos) {
        return;
    }
    let mut rate = u32::MAX;
    let mut ends = Vec::new();
    for &c in &pipes {
        if let Some(Kind::Pipe(_, t)) = kind_at(&*host, level, c) {
            rate = rate.min(family.rate(t));
        }
        for dir in Direction::ALL {
            let n = offset(c, dir);
            if !matches!(kind_at(&*host, level, n), Some(Kind::Pipe(..))) {
                ends.push((n, dir.opposite()));
            }
        }
    }
    match family {
        Family::Pipe => move_network(host, level, &FLUIDS, &ends, rate),
        Family::Tube => move_network(host, level, &CHEMICALS, &ends, rate),
        Family::Transporter => move_network(host, level, &ITEMS, &ends, rate),
    }
}

/// A cube gives to the machines next to it (its cables take from it themselves).
fn tick_cube(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], tier: Tier, energy: i32) {
    if energy == 0 {
        return;
    }
    let mut budget = tier.cube_rate().min(energy).max(0) as u32;
    for dir in Direction::ALL {
        let n = offset(pos, dir);
        if budget == 0 {
            break;
        }
        if matches!(kind_at(&*host, level, n), Some(Kind::Machine(_) | Kind::Factory(..))) {
            budget -= route(host, level, (pos, dir), (n, dir.opposite()), budget);
        }
    }
}

/// Whether `stack`'s result fits in `held`.
fn fits(host: &dyn Host, held: &Option<ItemStack>, id: &str, count: u32) -> Option<ItemStack> {
    let key = host.item_key(&Identifier::parse(id).ok()?)?;
    let out = ItemStack::new(key, count)?;
    match held {
        None => Some(out),
        Some(h) => {
            let max = host.max_stack_size(h.item()).unwrap_or(64);
            (h.is_stackable_with(&out) && h.count() + count <= max).then_some(out)
        }
    }
}

/// The infuser's tank is this many units full at most.
const INFUSE_CAPACITY: u32 = 1000;

fn text(nbt: &Compound, key: &str) -> Option<String> {
    match nbt.get(&key.into()) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()).filter(|s| !s.is_empty()),
        _ => None,
    }
}

fn single(id: &str, n: u32) -> Recipe {
    Recipe { output: id.to_owned(), count: n, take: [1, 0], infuse: None }
}

/// What `machine` makes of its inputs, if it has a recipe: `input` is the first, `second` the
/// second (combiner, infuser).
fn recipe(
    host: &mut dyn ServerHost,
    level: LevelRef,
    machine: Machine,
    input: &ItemStack,
    second: Option<&ItemStack>,
    nbt: &Compound,
) -> Option<Recipe> {
    let id = input.item().id(&*host);
    match machine {
        Machine::EnrichmentChamber => enrich(&id).map(|(o, n)| single(&o, n)),
        Machine::Crusher => crush(&id).map(|(o, n)| single(&o, n)),
        Machine::PurificationChamber => purify(&id).map(|(o, n)| single(&o, n)),
        Machine::ChemicalInjectionChamber => inject(&id).map(|(o, n)| single(&o, n)),
        Machine::ElectrolyticSeparator | Machine::ChemicalInfuser => None,
        Machine::PrecisionSawmill => saw(&id),
        Machine::Combiner => combine(&id, &second?.item().id(&*host)),
        Machine::MetallurgicInfuser => {
            infuse(&id, &text(nbt, "infuse_type")?, int(nbt, "infuse_amount").max(0) as u32)
        }
        Machine::EnergizedSmelter => {
            let kind = Identifier::parse("minecraft:smelting").ok()?;
            let m = level
                .recipe_for(host, &kind, &RecipeInput::Single(input.clone()))
                .ok()
                .flatten()?;
            Some(single(&m.output.item().id(&*host), m.output.count()))
        }
    }
}

/// The infuser takes an infusing item from its second slot into the tank when there is room.
fn absorb_infuse(host: &dyn Host, nbt: &mut Compound, items: &mut [Option<ItemStack>]) {
    let Some(stack) = items[INPUT2].as_ref() else { return };
    let Some((ty, n)) = infuse_of(&stack.item().id(host)) else { return };
    let held = int(nbt, "infuse_amount").max(0) as u32;
    let current = text(nbt, "infuse_type");
    if held > 0 && current.as_deref() != Some(ty) || held + n > INFUSE_CAPACITY {
        return;
    }
    nbt.insert("infuse_type".into(), Tag::String(ty.into()));
    set_int(nbt, "infuse_amount", (held + n) as i32);
    items[INPUT2] = items[INPUT2].take().and_then(|i| i.shrink(1));
}

/// The chemical tanks a machine has, by index: what pipes reach through `mekanism:chemicals`.
fn tank_specs(m: Machine) -> &'static [TankSpec] {
    const BIG: u32 = 10_000;
    const fn spec(name: &'static str, insert: bool, only: Option<&'static str>) -> TankSpec {
        TankSpec { name, capacity: BIG, insert, extract: !insert, only }
    }
    const PURIFY: [TankSpec; 1] = [spec("in", true, Some("mekanism:oxygen"))];
    const INJECT: [TankSpec; 1] = [spec("in", true, Some("mekanism:hydrogen_chloride"))];
    const SEPARATE: [TankSpec; 2] = [spec("first", false, None), spec("second", false, None)];
    const INFUSE: [TankSpec; 3] =
        [spec("left", true, None), spec("right", true, None), spec("out", false, None)];
    match m {
        Machine::PurificationChamber => &PURIFY,
        Machine::ChemicalInjectionChamber => &INJECT,
        Machine::ElectrolyticSeparator => &SEPARATE,
        Machine::ChemicalInfuser => &INFUSE,
        _ => &[],
    }
}

/// Whether the machine's chemical tank holds what an operation spends this tick.
fn has_chemical(m: Machine, nbt: &Compound) -> bool {
    match m.chemical_use() {
        None => true,
        Some((chem, per)) => {
            let t = Tank::load(nbt, "in");
            t.chemical.as_deref() == Some(chem) && t.amount >= per
        }
    }
}

/// The first tank of a `ModFluidTanks` block, as the game saves it: (fluid id, amount).
fn fluid_tank(nbt: &Compound) -> (Option<String>, i32) {
    let Some(Tag::List(list)) = nbt.get(&"fluids".into()) else { return (None, 0) };
    match list.first() {
        Some(Tag::Compound(t)) => {
            let fluid = match t.get(&"Fluid".into()) {
                Some(Tag::String(s)) => Some(s.to_string_lossy()).filter(|f| f != "minecraft:empty"),
                _ => None,
            };
            (fluid, int(t, "Amount"))
        }
        _ => (None, 0),
    }
}

fn set_fluid_tank(nbt: &mut Compound, fluid: Option<&str>, amount: i32) {
    let mut t = Compound::new();
    let fluid = if amount > 0 { fluid.unwrap_or("minecraft:empty") } else { "minecraft:empty" };
    t.insert("Fluid".into(), Tag::String(fluid.into()));
    t.insert("Amount".into(), Tag::Int(amount.max(0)));
    nbt.insert("fluids".into(), Tag::List(vec![Tag::Compound(t)]));
}

/// One tick of the machines that work on fluids and chemicals: the separator splits a fluid
/// into two chemicals, the chemical infuser joins two into one. They spend energy every tick
/// they make something.
fn tick_chemical(m: Machine, nbt: &mut Compound) {
    let energy = int(nbt, "energy");
    let need = m.fe_per_tick();
    if energy < need {
        return;
    }
    let made = match m {
        Machine::ElectrolyticSeparator => {
            let (fluid, amount) = fluid_tank(nbt);
            let Some((cost, (c1, n1), (c2, n2))) = fluid.as_deref().and_then(electrolyse) else {
                return;
            };
            let (mut t1, mut t2) = (Tank::load(nbt, "first"), Tank::load(nbt, "second"));
            if (amount as u32) < cost {
                return;
            }
            // each output tank holds one chemical at a time: both must have room for this tick
            let fits1 = t1.amount == 0 || t1.chemical.as_deref() == Some(c1);
            let fits2 = t2.amount == 0 || t2.chemical.as_deref() == Some(c2);
            if !fits1 || !fits2 || t1.amount + n1 > 10_000 || t2.amount + n2 > 10_000 {
                return;
            }
            t1.fill(c1, n1, 10_000);
            t2.fill(c2, n2, 10_000);
            t1.save(nbt, "first");
            t2.save(nbt, "second");
            set_fluid_tank(nbt, fluid.as_deref(), amount - cost as i32);
            true
        }
        Machine::ChemicalInfuser => {
            let (mut l, mut r, mut out) =
                (Tank::load(nbt, "left"), Tank::load(nbt, "right"), Tank::load(nbt, "out"));
            let (Some(a), Some(b)) = (l.chemical.clone(), r.chemical.clone()) else { return };
            let Some(result) = combine_chemicals(&a, &b) else { return };
            if l.amount == 0 || r.amount == 0 || out.fill(result, 1, 10_000) == 0 {
                return;
            }
            l.drain(&a, 1);
            r.drain(&b, 1);
            l.save(nbt, "left");
            r.save(nbt, "right");
            out.save(nbt, "out");
            true
        }
        _ => false,
    };
    if made {
        set_int(nbt, "energy", energy - need);
    }
}

/// One tick of a factory: `lanes` inputs (slots 0..lanes), their outputs (lanes..2*lanes), then the
/// energy item and two upgrades. Every lane runs its own operation, all spending from one buffer
/// and one chemical tank.
fn tick_factory(
    host: &mut dyn ServerHost,
    level: LevelRef,
    tier: Tier,
    machine: Machine,
    nbt: &mut Compound,
    items: &mut Vec<Option<ItemStack>>,
) {
    let lanes = tier.lanes();
    if items.len() < lanes * 2 + 3 {
        items.resize(lanes * 2 + 3, None);
    }
    let mut energy = int(nbt, "energy");
    for lane in 0..lanes {
        let out = lanes + lane;
        let key = format!("progress_{lane}");
        let mut work = Work { progress: int(nbt, &key) };
        let found = items[lane]
            .clone()
            .and_then(|input| recipe(host, level, machine, &input, None, nbt));
        let result = found.as_ref().and_then(|r| {
            if items[lane].as_ref().is_none_or(|i| i.count() < r.take[0]) {
                return None;
            }
            fits(&*host, &items[out], &r.output, r.count)
        });
        let result = result.filter(|_| has_chemical(machine, nbt));
        let s = step(&mut work, result.is_some(), energy, machine.fe_per_tick(), machine.ticks());
        energy -= s.spent;
        if let (Some((chem, per)), true) = (machine.chemical_use(), s.spent > 0) {
            let mut tank = Tank::load(nbt, "in");
            tank.drain(chem, per);
            tank.save(nbt, "in");
        }
        set_int(nbt, &key, work.progress);
        if s.finished {
            if let (Some(made), Some(r)) = (result, found) {
                items[out] = match items[out].take() {
                    None => Some(made),
                    Some(mut held) => {
                        let _ = held.grow(made.count());
                        Some(held)
                    }
                };
                items[lane] = items[lane].take().and_then(|i| i.shrink(r.take[0]));
            }
        }
    }
    set_int(nbt, "energy", energy);
}

fn tick_machine(
    host: &mut dyn ServerHost,
    level: LevelRef,
    machine: Machine,
    nbt: &mut Compound,
    items: &mut Vec<Option<ItemStack>>,
) {
    if items.len() < machine.slots() {
        items.resize(machine.slots(), None);
    }
    if machine == Machine::MetallurgicInfuser {
        absorb_infuse(&*host, nbt, items);
    }
    let mut work = Work { progress: int(nbt, "progress") };
    let second = items.get(INPUT2).cloned().flatten().filter(|_| machine.second_input());
    let found = items[INPUT]
        .clone()
        .and_then(|input| recipe(host, level, machine, &input, second.as_ref(), nbt));
    // the stack the recipe makes, if it fits and both inputs hold what it takes
    let result = found.as_ref().and_then(|r| {
        let enough = items[INPUT].as_ref().is_some_and(|i| i.count() >= r.take[0])
            && (r.take[1] == 0 || items[INPUT2].as_ref().is_some_and(|i| i.count() >= r.take[1]));
        if !enough {
            return None;
        }
        fits(&*host, &items[OUTPUT], &r.output, r.count)
    });
    let result = result.filter(|_| has_chemical(machine, nbt));
    let energy = int(nbt, "energy");
    let s = step(&mut work, result.is_some(), energy, machine.fe_per_tick(), machine.ticks());
    if let (Some((chem, per)), true) = (machine.chemical_use(), s.spent > 0) {
        let mut tank = Tank::load(nbt, "in");
        tank.drain(chem, per);
        tank.save(nbt, "in");
    }
    set_int(nbt, "energy", energy - s.spent);
    set_int(nbt, "progress", work.progress);
    if s.finished {
        if let (Some(out), Some(r)) = (result, found) {
            items[OUTPUT] = match items[OUTPUT].take() {
                None => Some(out),
                Some(mut held) => {
                    let _ = held.grow(out.count());
                    Some(held)
                }
            };
            items[INPUT] = items[INPUT].take().and_then(|i| i.shrink(r.take[0]));
            if r.take[1] > 0 {
                items[INPUT2] = items[INPUT2].take().and_then(|i| i.shrink(r.take[1]));
            }
            if let Some((_, cost)) = r.infuse {
                let left = (int(nbt, "infuse_amount") - cost as i32).max(0);
                set_int(nbt, "infuse_amount", left);
                if left == 0 {
                    nbt.remove(&"infuse_type".into());
                }
            }
        }
    }
}

/// One part for every Mekanism block entity; the block's id says which kind it is.
struct Machines;

#[mod_api::horizon_part]
impl BlockPart for Machines {}

#[mod_api::horizon_part]
impl BlockEntityPart for Machines {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        parse_block(event.block).map(|_| {
            let mut nbt = Compound::new();
            set_int(&mut nbt, "energy", 0);
            nbt
        })
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if parse_block(event.block).is_some() {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let Some(level) = host.level(host.dimension()) else { return };
        let Some(kind) = kind_at(&*host, level, pos) else { return };
        let mut nbt = event.entity.nbt.clone();
        let mut items = event.entity.items.clone();
        match kind {
            Kind::Cable(t) => tick_cable(host, level, pos, t),
            Kind::Pipe(f, _) => tick_pipe(host, level, pos, f),
            Kind::Factory(t, m) => tick_factory(host, level, t, m, &mut nbt, &mut items),
            Kind::Cube(t) => tick_cube(host, level, pos, t, int(&nbt, "energy")),
            Kind::Machine(m) => {
                if m.has_items() {
                    tick_machine(host, level, m, &mut nbt, &mut items);
                } else {
                    tick_chemical(m, &mut nbt);
                }
                // the furnace template's `lit` shows the machine working
                let lit = i32::from(int(&nbt, "progress") > 0);
                if int(&nbt, "lit") != lit {
                    set_int(&mut nbt, "lit", lit);
                    let _ = host.set_block_property(pos, "lit", if lit == 1 { "true" } else { "false" });
                }
            }
        }
        if nbt == event.entity.nbt && items == event.entity.items {
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        }
        event.entity.nbt = nbt;
        event.entity.items = items;
    }
}

/// The game's energy buffer for a block: cubes and cables move energy both ways through every
/// face, machines only take it.
fn buffer_of(kind: Kind) -> ModEnergyBuffer {
    let all = [true; 6];
    let none = [false; 6];
    match kind {
        Kind::Cube(t) => ModEnergyBuffer {
            capacity: t.cube_capacity(),
            max_insert: t.cube_rate(),
            max_extract: t.cube_rate(),
            insert_faces: all,
            extract_faces: all,
        },
        // a cable stores nothing; its network moves the energy
        Kind::Cable(_) => ModEnergyBuffer {
            capacity: 0,
            max_insert: 0,
            max_extract: 0,
            insert_faces: none,
            extract_faces: none,
        },
        Kind::Pipe(..) => ModEnergyBuffer::default(),
        Kind::Factory(t, m) => {
            let buffer = m.buffer() * t.lanes() as i32;
            ModEnergyBuffer {
                capacity: buffer,
                max_insert: buffer,
                max_extract: 0,
                insert_faces: all,
                extract_faces: none,
            }
        }
        Kind::Machine(m) => ModEnergyBuffer {
            capacity: m.buffer(),
            max_insert: m.buffer(),
            max_extract: 0,
            insert_faces: all,
            extract_faces: none,
        },
    }
}

/// A factory's slots: inputs, outputs, an energy item, two upgrades, shown by a 3x3-row chest
/// menu. Hoppers feed the inputs from above and the sides, and take from the outputs.
fn factory_container(t: Tier) -> ModContainer {
    let lanes = t.lanes();
    let inputs: Vec<usize> = (0..lanes).collect();
    let outputs: Vec<usize> = (lanes..lanes * 2).collect();
    ModContainer {
        slots: lanes * 2 + 3,
        faces: [
            outputs.clone(),
            inputs.clone(),
            inputs.clone(),
            inputs.clone(),
            inputs.clone(),
            inputs.clone(),
        ],
        inputs,
        outputs,
        menu: Some("minecraft:generic_9x3".into()),
    }
}

/// The slots a machine keeps (input, output, an energy item, two upgrades, and for two-input
/// machines the second input); vanilla's hopper or 3x3 menu shows them until Mekanism's own
/// screens exist. Hoppers feed the inputs and empty the output only.
fn container_of(m: Machine) -> ModContainer {
    let mut inputs = vec![INPUT, 2, 3, 4];
    let top = if m.second_input() {
        inputs.push(INPUT2);
        vec![INPUT, INPUT2]
    } else {
        vec![INPUT]
    };
    ModContainer {
        slots: m.slots(),
        faces: [vec![OUTPUT], top.clone(), top.clone(), top.clone(), top.clone(), top],
        inputs,
        outputs: vec![OUTPUT],
        menu: Some(m.menu().into()),
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let mut groups: Vec<(&str, Vec<String>)> = vec![
            ("energy_cube", Tier::ALL.iter().map(|t| cube_id(*t)).collect()),
            ("universal_cable", Tier::ALL.iter().map(|t| cable_id(*t)).collect()),
        ];
        for f in Family::ALL {
            groups.push((f.entity_type(), Tier::ALL.iter().map(|t| pipe_id(f, *t)).collect()));
        }
        for m in Machine::ALL {
            if m.factory_name().is_some() {
                groups.push((
                    m.factory_name().unwrap_or(""),
                    Tier::ALL.iter().filter_map(|t| factory_id(*t, m)).collect(),
                ));
            }
        }
        for m in Machine::ALL {
            groups.push((m.name(), vec![format!("mekanism:{}", m.name())]));
        }
        matrix::register(r);
        tank::register(r);
        steam::register(r);
        for (entity_type, ids) in groups {
            for id in &ids {
                let Some(kind) = parse_block(id) else { continue };
                let key = r.block_key(id);
                if !matches!(kind, Kind::Cable(_) | Kind::Pipe(..)) {
                    r.block_energy(&key, buffer_of(kind));
                }
                if let Kind::Factory(t, _) = kind {
                    r.block_container(&key, factory_container(t));
                }
                if let Kind::Machine(m) = kind {
                    if m.has_items() {
                        r.block_container(&key, container_of(m));
                    }
                    if m == Machine::ElectrolyticSeparator {
                        // water or brine in from any side, 10 buckets
                        let faces = std::array::from_fn(|_| vec![0]);
                        r.block_fluids(&key, ModFluidTanks::new(vec![10_000], faces, vec![0], vec![]));
                    }
                }
            }
            let names: Vec<&str> = ids.iter().map(String::as_str).collect();
            let built = r.block_entity_type(entity_type).blocks(&names).build();
            let _ = r.block_entity_part(&built, Machines);
            let machine = match ids.first().and_then(|id| parse_block(id)) {
                Some(Kind::Machine(m) | Kind::Factory(_, m)) => Some(m),
                _ => None,
            };
            if let Some(m) = machine {
                let specs = tank_specs(m);
                if !specs.is_empty() {
                    r.block_entity_capability(&CHEMICALS, &built, move |_, at| {
                        Some(Box::new(BlockTanks { at, specs }))
                    });
                }
            }
        }
    }
}
