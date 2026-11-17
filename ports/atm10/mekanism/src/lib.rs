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
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Compound, Direction, ENERGY, Hooks, Host, Identifier,
    HostTransaction, ItemStack, LevelRef, LevelSide, ModContainer, ModEnergyBuffer, RecipeInput, Registrar,
    RegistrationContext, ServerHost, Tag,
};

pub mod chemical;
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
    let mut seen = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(pos) = queue.pop_front() {
        if seen.len() >= 4096 {
            break;
        }
        for dir in Direction::ALL {
            let n = offset(pos, dir);
            if !seen.contains(&n) && matches!(kind_at(host, level, n), Some(Kind::Cable(_))) {
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
                Some(Kind::Cable(_)) => {}
                Some(Kind::Cube(_)) => {
                    sources.push((n, dir.opposite()));
                    cubes.push((n, dir.opposite()));
                }
                Some(Kind::Machine(_)) => sinks.push((n, dir.opposite())),
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
        if matches!(kind_at(&*host, level, n), Some(Kind::Machine(_))) {
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

/// What `machine` makes of its inputs, if it has a recipe.
fn recipe(
    host: &mut dyn ServerHost,
    level: LevelRef,
    machine: Machine,
    items: &[Option<ItemStack>],
    nbt: &Compound,
) -> Option<Recipe> {
    let input = items[INPUT].as_ref()?;
    let id = input.item().id(&*host);
    match machine {
        Machine::EnrichmentChamber => enrich(&id).map(|(o, n)| single(&o, n)),
        Machine::Crusher => crush(&id).map(|(o, n)| single(&o, n)),
        Machine::PrecisionSawmill => saw(&id),
        Machine::Combiner => combine(&id, &items[INPUT2].as_ref()?.item().id(&*host)),
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
    let found = recipe(host, level, machine, items, nbt);
    // the stack the recipe makes, if it fits and both inputs hold what it takes
    let result = found.as_ref().and_then(|r| {
        let enough = items[INPUT].as_ref().is_some_and(|i| i.count() >= r.take[0])
            && (r.take[1] == 0 || items[INPUT2].as_ref().is_some_and(|i| i.count() >= r.take[1]));
        if !enough {
            return None;
        }
        fits(&*host, &items[OUTPUT], &r.output, r.count)
    });
    let energy = int(nbt, "energy");
    let s = step(&mut work, result.is_some(), energy, machine.fe_per_tick(), machine.ticks());
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
            Kind::Cube(t) => tick_cube(host, level, pos, t, int(&nbt, "energy")),
            Kind::Machine(m) => {
                tick_machine(host, level, m, &mut nbt, &mut items);
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
        Kind::Machine(m) => ModEnergyBuffer {
            capacity: m.buffer(),
            max_insert: m.buffer(),
            max_extract: 0,
            insert_faces: all,
            extract_faces: none,
        },
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
        for m in Machine::ALL {
            groups.push((m.name(), vec![format!("mekanism:{}", m.name())]));
        }
        for (entity_type, ids) in groups {
            for id in &ids {
                let Some(kind) = parse_block(id) else { continue };
                let key = r.block_key(id);
                if !matches!(kind, Kind::Cable(_)) {
                    r.block_energy(&key, buffer_of(kind));
                }
                if let Kind::Machine(m) = kind {
                    r.block_container(&key, container_of(m));
                }
            }
            let names: Vec<&str> = ids.iter().map(String::as_str).collect();
            let built = r.block_entity_type(entity_type).blocks(&names).build();
            let _ = r.block_entity_part(&built, Machines);
        }
    }
}
