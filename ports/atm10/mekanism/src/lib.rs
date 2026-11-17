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
pub mod evaporation;
pub mod fission;
pub mod fusion;
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
    // machines first, then cubes and the foreign blocks: they take what is left
    let machines = sinks.clone();
    sinks.extend(cubes);
    sinks.extend(foreign);
    let mut budget = rate.max(0) as u32;
    for &sink in &sinks {
        for &source in &sources {
            if budget == 0 {
                return;
            }
            if source.0 == sink.0 {
                continue;
            }
            let mut want = budget;
            if !machines.contains(&sink) {
                // two stores balance: energy runs from the fuller to the emptier, until both are
                // as full as each other, so it cannot swing back and forth
                let level_of = |host: &mut dyn ServerHost, end: ([i32; 3], Direction)| {
                    level
                        .capability(host, &ENERGY, end.0, Some(end.1))
                        .map(|c| (f64::from(c.amount(host)), f64::from(c.capacity(host))))
                };
                let (Some((sa, sc)), Some((da, dc))) = (level_of(host, source), level_of(host, sink)) else {
                    continue;
                };
                if sc <= 0.0 || dc <= 0.0 || sa * dc <= da * sc {
                    continue;
                }
                let balance = ((sa * dc - da * sc) / (sc + dc)).floor();
                want = want.min(balance.min(f64::from(u32::MAX)) as u32);
            }
            if want > 0 {
                budget -= route(host, level, source, sink, want).min(budget);
            }
        }
    }
}

/// Moves up to `want` from the tanks of `src` into those of `dst`, each resource to the tank that
/// takes it, by the amount that makes the two equally full: how much moved.
fn balance_tanks<R: 'static>(
    host: &mut dyn ServerHost,
    src: &dyn ResourceHandler<R>,
    dst: &dyn ResourceHandler<R>,
    want: u32,
) -> u32 {
    let (mut left, mut total) = (want, 0u32);
    for i in 0..src.size(host) {
        if left == 0 {
            break;
        }
        let Some(r) = src.resource(host, i) else { continue };
        let (sa, sc) = (f64::from(src.amount(host, i)), f64::from(src.capacity(host, i, &r)));
        if sc <= 0.0 {
            continue;
        }
        let Some(j) = (0..dst.size(host)).find(|j| dst.capacity(host, *j, &r) > 0) else { continue };
        let (da, dc) = (f64::from(dst.amount(host, j)), f64::from(dst.capacity(host, j, &r)));
        if sa * dc <= da * sc {
            continue;
        }
        let balance = ((sa * dc - da * sc) / (sc + dc)).floor();
        let n = left.min(balance.min(f64::from(u32::MAX)) as u32);
        if n == 0 {
            continue;
        }
        // all or nothing; a store that takes only part (a cauldron gives whole thirds) is
        // asked again for that part
        let attempt = |host: &mut dyn ServerHost, n: u32| {
            host.transaction(|host, tx| {
                let out = src.extract(host, tx, i, &r, n);
                if out == 0 {
                    return Ok(0);
                }
                let took = dst.insert(host, tx, j, &r, out);
                if took < out { Err(took) } else { Ok(out) }
            })
        };
        let moved = match attempt(host, n) {
            Ok(m) => m,
            Err(took) if took > 0 => attempt(host, took).unwrap_or(0),
            Err(_) => 0,
        };
        total += moved;
        left -= moved;
    }
    total
}

/// How long (in ticks) a pair of endpoints keeps its direction: once one fed the other, the other
/// may not feed it back for this long, whatever the levels say.
const SETTLE: i32 = 200;

/// The pipe network's memory of the last move of each pair, in the lowest pipe's saved data.
struct Memory<'a> {
    nbt: &'a mut Compound,
    now: i32,
}

impl Memory<'_> {
    fn key(from: [i32; 3], to: [i32; 3]) -> String {
        format!("m{}_{}_{}>{}_{}_{}", from[0], from[1], from[2], to[0], to[1], to[2])
    }

    /// Whether the pair `from` to `to` may move: not if the other way moved lately.
    fn allows(&self, from: [i32; 3], to: [i32; 3]) -> bool {
        let last = int(self.nbt, &Self::key(to, from));
        last == 0 || self.now - last >= SETTLE
    }

    fn moved(&mut self, from: [i32; 3], to: [i32; 3]) {
        set_int(self.nbt, &Self::key(from, to), self.now);
    }
}

/// How full an endpoint is, 0 to 1: occupied slots of an item handler, the fullest tank of a fluid
/// or chemical one. An endpoint with nothing in it to be full of counts as full, so nothing moves
/// into it.
fn fill_of<R: 'static>(host: &mut dyn ServerHost, cap: &dyn ResourceHandler<R>, items: bool) -> f64 {
    let n = cap.size(host);
    if n == 0 {
        return 1.0;
    }
    let mut best = 0.0f64;
    let mut occupied = 0usize;
    for i in 0..n {
        let Some(r) = cap.resource(host, i) else { continue };
        occupied += 1;
        let amount = f64::from(cap.amount(host, i));
        let room = f64::from(cap.capacity(host, i, &r)).max(1.0);
        best = best.max((amount / room).min(1.0));
    }
    if items { occupied as f64 / n as f64 } else { best }
}

/// Moves up to `budget` of `R` between the endpoints of a pipe network: out of each endpoint,
/// into the others, in one transaction per pair. Between two stores (chests, tanks) it runs from
/// the fuller to the emptier only, so it cannot swing back and forth; a machine's inputs and
/// outputs are one-way already, so those are moved freely.
fn move_network<R: 'static>(
    host: &mut dyn ServerHost,
    level: LevelRef,
    key: &CapabilityKey<dyn ResourceHandler<R>>,
    ends: &[([i32; 3], Direction)],
    mut budget: u32,
    items: bool,
    memory: &mut Memory<'_>,
) {
    // machines and the multiblocks that only consume or produce (boiler, turbine, reactor) are
    // filled and emptied freely: their sides are one-way
    let machine_end = |host: &dyn Host, p: [i32; 3]| {
        matches!(kind_at(host, level, p), Some(Kind::Machine(_) | Kind::Factory(..)))
            || matches!(
                mb::id_at(host, level, p).as_deref(),
                Some(steam::BOILER_VALVE | steam::TURBINE_VALVE | fission::PORT | evaporation::VALVE | fusion::PORT)
            )
    };
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
            let want = budget;
            let stores = !machine_end(&*host, from.0) && !machine_end(&*host, to.0);
            if stores && !memory.allows(from.0, to.0) {
                continue;
            }
            if stores && !items {
                // two tanks balance, each chemical or fluid on its own: the amount that leaves
                // them equally full, so nothing swings back and forth
                let moved = balance_tanks(host, &*src, &*dst, want);
                if moved > 0 {
                    memory.moved(from.0, to.0);
                    budget -= moved.min(budget);
                }
                continue;
            }
            if stores {
                let (a, b) = (fill_of(host, &*src, true), fill_of(host, &*dst, true));
                if a <= b {
                    continue;
                }
            }
            let moved = host
                .transaction(|host, tx| {
                    move_resources(host, &src, &dst, |_| true, want, tx).map_err(|_| ())
                })
                .unwrap_or(0);
            if moved > 0 && stores {
                memory.moved(from.0, to.0);
            }
            budget -= moved.min(budget);
        }
    }
}

/// One tick of the pipe network `pos` belongs to, run only by its lowest pipe: what the blocks
/// touching it give is moved to the others, at the lowest tier's rate.
fn tick_pipe(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], family: Family, nbt: &mut Compound) {
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
    // a clock for the network's memory of which way each pair last moved
    let now = int(nbt, "clock") + 1;
    set_int(nbt, "clock", now);
    let mut memory = Memory { nbt, now };
    match family {
        Family::Pipe => move_network(host, level, &FLUIDS, &ends, rate, false, &mut memory),
        Family::Tube => move_network(host, level, &CHEMICALS, &ends, rate, false, &mut memory),
        Family::Transporter => move_network(host, level, &ITEMS, &ends, rate, true, &mut memory),
    }
}

/// A cube gives to the machines next to it (its cables take from it themselves). It moves its
/// own saved energy rather than asking its own capability, which cannot be read in its own tick.
fn tick_cube(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], tier: Tier, nbt: &mut Compound) {
    let energy = int(nbt, "energy");
    if energy <= 0 {
        return;
    }
    let mut budget = tier.cube_rate().min(energy).max(0) as u32;
    let mut given = 0u32;
    for dir in Direction::ALL {
        if budget == 0 {
            break;
        }
        let n = offset(pos, dir);
        if !matches!(kind_at(&*host, level, n), Some(Kind::Machine(_) | Kind::Factory(..))) {
            continue;
        }
        let Some(cap) = level.capability(host, &ENERGY, n, Some(dir.opposite())) else { continue };
        let moved = host
            .transaction(|host, tx| Ok::<u32, ()>(cap.insert(host, tx, budget)))
            .unwrap_or(0);
        budget -= moved.min(budget);
        given += moved;
    }
    set_int(nbt, "energy", energy - given as i32);
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
        Machine::ElectrolyticSeparator | Machine::ChemicalInfuser | Machine::IsotopicCentrifuge => None,
        Machine::ChemicalOxidizer => None,
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
    const OXIDIZE: [TankSpec; 1] = [spec("out", false, None)];
    const CENTRIFUGE: [TankSpec; 2] =
        [spec("in", true, Some("mekanism:uranium_hexafluoride")), spec("out", false, None)];
    const INFUSE: [TankSpec; 3] =
        [spec("left", true, None), spec("right", true, None), spec("out", false, None)];
    match m {
        Machine::PurificationChamber => &PURIFY,
        Machine::ChemicalInjectionChamber => &INJECT,
        Machine::ElectrolyticSeparator => &SEPARATE,
        Machine::ChemicalInfuser => &INFUSE,
        Machine::ChemicalOxidizer => &OXIDIZE,
        Machine::IsotopicCentrifuge => &CENTRIFUGE,
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
        Machine::IsotopicCentrifuge => {
            let (mut inp, mut out) = (Tank::load(nbt, "in"), Tank::load(nbt, "out"));
            let Some(c) = inp.chemical.clone() else { return };
            let Some(result) = centrifuge(&c) else { return };
            // 5 mB a tick
            let n = inp.amount.min(5);
            if n == 0 || out.fill(result, n, 10_000) != n {
                return;
            }
            inp.drain(&c, n);
            inp.save(nbt, "in");
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
    if items.len() < FACTORY_SLOTS {
        items.resize(FACTORY_SLOTS, None);
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

/// The oxidizer turns one item at a time into a chemical (200 ticks of work, as the other
/// machines), when the output tank has room and holds the same chemical or is empty.
fn tick_oxidizer(host: &dyn Host, nbt: &mut Compound, items: &mut [Option<ItemStack>]) {
    let energy = int(nbt, "energy");
    let need = Machine::ChemicalOxidizer.fe_per_tick();
    let made = items[INPUT].as_ref().and_then(|i| oxidize(&i.item().id(host)));
    let mut out = Tank::load(nbt, "out");
    let ok = made.is_some_and(|(chem, n)| {
        (out.amount == 0 || out.chemical.as_deref() == Some(chem)) && out.amount + n <= 10_000
    });
    let mut work = Work { progress: int(nbt, "progress") };
    let s = step(&mut work, ok, energy, need, Machine::ChemicalOxidizer.ticks());
    set_int(nbt, "energy", energy - s.spent);
    set_int(nbt, "progress", work.progress);
    if let (true, Some((chem, n))) = (s.finished, made) {
        out.fill(chem, n, 10_000);
        out.save(nbt, "out");
        items[INPUT] = items[INPUT].take().and_then(|i| i.shrink(1));
    }
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
    if machine == Machine::ChemicalOxidizer {
        tick_oxidizer(host, nbt, items);
        return;
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
            Kind::Pipe(f, _) => tick_pipe(host, level, pos, f, &mut nbt),
            Kind::Factory(t, m) => {
                tick_factory(host, level, t, m, &mut nbt, &mut items);
                let lit = i32::from((0..t.lanes()).any(|l| int(&nbt, &format!("progress_{l}")) > 0));
                if int(&nbt, "lit") != lit {
                    set_int(&mut nbt, "lit", lit);
                    let _ = host.set_block_property(pos, "lit", if lit == 1 { "true" } else { "false" });
                }
            }
            Kind::Cube(t) => tick_cube(host, level, pos, t, &mut nbt),
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

/// The 3-row chest menu's slots.
const FACTORY_SLOTS: usize = 27;

/// A factory's slots: inputs, outputs, an energy item, two upgrades, shown by a 3x3-row chest
/// menu. Hoppers feed the inputs from above and the sides, and take from the outputs.
fn factory_container(t: Tier) -> ModContainer {
    let lanes = t.lanes();
    let inputs: Vec<usize> = (0..lanes).collect();
    let outputs: Vec<usize> = (lanes..lanes * 2).collect();
    ModContainer {
        slots: FACTORY_SLOTS,
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
        fission::register(r);
        evaporation::register(r);
        fusion::register(r);
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
