//! Create for Horizon, first slice: rotation and stress. Shafts, cogwheels, large cogwheels and
//! gearboxes carry a network's speed, a water wheel, a windmill bearing and a creative motor make
//! it, and the network's lowest block solves it (see `kinetics.rs`) and tells every block of it
//! its speed through the block entity's saved `speed` (RPM), `capacity`, `stress` and a
//! `jammed` / `overstressed` flag.
//!
//! Written from Create's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What is ported and which numbers still need a check against
//! the NeoForge original is in README.md.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Compound, Hooks, Host, HostBlockStates, HostTransaction, ItemStack, LevelRef,
    LevelSide, ModContainer, Registrar, RegistrationContext, ServerHost, Tag,
};

pub mod kinetics;
pub mod rules;
use kinetics::{Axis, Node, Part, Pos, solve};
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

/// The id and the property reader of the block at `pos`.
fn state_at(host: &dyn Host, level: LevelRef, pos: Pos) -> Option<(String, Option<Axis>, Option<[i32; 3]>, i32)> {
    let state = level.block_state(host, pos)?;
    let id = state.block().id(host).to_string();
    let meta = host.block_states();
    let prop = |name: &str| state.property(&meta, name).ok().flatten().map(str::to_owned);
    let axis = prop("axis").and_then(|a| Axis::from_name(&a));
    let facing = prop("facing").and_then(|f| facing_offset(&f));
    let level_prop = prop("level").and_then(|l| l.parse().ok()).unwrap_or(0);
    Some((id, axis, facing, level_prop))
}

/// What a facing name points at.
fn facing_offset(name: &str) -> Option<[i32; 3]> {
    Some(match name {
        "east" => [1, 0, 0],
        "west" => [-1, 0, 0],
        "up" => [0, 1, 0],
        "down" => [0, -1, 0],
        "south" => [0, 0, 1],
        "north" => [0, 0, -1],
        _ => return None,
    })
}

/// What the block at `pos` is in a kinetic network, if it is part of one.
fn node_at(host: &dyn Host, level: LevelRef, pos: Pos) -> Option<Node> {
    let (id, axis, facing, _) = state_at(host, level, pos)?;
    let kind = parse_block(&id)?;
    let axis = axis
        .or_else(|| facing.and_then(Axis::of_offset))
        .unwrap_or(Axis::Y);
    let part = match kind {
        Kind::Shaft => Part::Shaft,
        Kind::Cogwheel => Part::Cog,
        Kind::LargeCogwheel => Part::LargeCog,
        Kind::Gearbox => Part::Gearbox,
        Kind::Millstone => Part::Consumer { impact: MILLSTONE_IMPACT },
        Kind::MechanicalPress => Part::Consumer { impact: PRESS_IMPACT },
        Kind::MechanicalMixer => Part::Consumer { impact: MIXER_IMPACT },
        Kind::Belt => Part::Consumer { impact: BELT_IMPACT },
        Kind::Depot | Kind::Basin => return None,
        Kind::CreativeMotor => Part::Source { rpm: CREATIVE_MOTOR_RPM, capacity: CREATIVE_MOTOR_CAPACITY },
        Kind::WaterWheel => {
            let flowing = Direction6::ALL
                .iter()
                .filter(|d| Axis::of_offset(**d) != Some(axis))
                .filter(|d| {
                    let at = [pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]];
                    state_at(host, level, at).is_some_and(|(id, _, _, lvl)| id == "minecraft:water" && lvl != 0)
                })
                .count();
            Part::Source { rpm: water_wheel_rpm(flowing), capacity: WATER_WHEEL_CAPACITY }
        }
        Kind::WindmillBearing => {
            let toward = facing.unwrap_or([0, 1, 0]);
            let sails = count_sails(host, level, pos, toward);
            Part::Source { rpm: windmill_rpm(sails), capacity: WINDMILL_CAPACITY }
        }
    };
    Some(Node::new(part, axis))
}

/// The six unit offsets.
struct Direction6;

impl Direction6 {
    const ALL: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
}

/// The sail blocks (wool, carpet, glass-free frames: `is_sail`) joined to the block in front of
/// the bearing, up to `MAX_SAILS`.
fn count_sails(host: &dyn Host, level: LevelRef, bearing: Pos, toward: [i32; 3]) -> usize {
    let start = [bearing[0] + toward[0], bearing[1] + toward[1], bearing[2] + toward[2]];
    let mut seen = std::collections::BTreeSet::new();
    let mut queue = std::collections::VecDeque::from([start]);
    while let Some(at) = queue.pop_front() {
        if seen.len() >= MAX_SAILS || !seen.insert(at) {
            continue;
        }
        let is = state_at(host, level, at).is_some_and(|(id, ..)| is_sail(&id));
        if !is {
            seen.remove(&at);
            continue;
        }
        for d in Direction6::ALL {
            let n = [at[0] + d[0], at[1] + d[1], at[2] + d[2]];
            if !seen.contains(&n) {
                queue.push_back(n);
            }
        }
    }
    seen.len()
}

/// One part for every Create block entity; the block's id says which kind it is.
struct Kinetic;

#[mod_api::horizon_part]
impl BlockPart for Kinetic {}

#[mod_api::horizon_part]
impl BlockEntityPart for Kinetic {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        parse_block(event.block).map(|_| {
            let mut nbt = Compound::new();
            set_int(&mut nbt, "speed_x1000", 0);
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
        let Some(kind) = state_at(&*host, level, pos).and_then(|(id, ..)| parse_block(&id)) else { return };
        let mut nbt = event.entity.nbt.clone();
        let mut items = event.entity.items.clone();
        kinetic_step(host, level, pos, &mut nbt);
        let speed = f64::from(int(&nbt, "speed_x1000")) / 1000.0;
        match kind {
            Kind::Millstone => tick_millstone(host, speed, &mut nbt, &mut items),
            Kind::MechanicalPress => tick_press(host, level, pos, speed, &mut nbt),
            Kind::Belt => tick_belt(host, level, pos, speed, &mut nbt, &mut items),
            Kind::MechanicalMixer => tick_mixer(host, level, pos, speed, &mut nbt),
            _ => {}
        }
        if nbt == event.entity.nbt && items == event.entity.items {
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        }
        event.entity.nbt = nbt;
        event.entity.items = items;
    }
}

/// Solves the block's network when it is this block's turn and nobody keeps it up to date, and
/// writes the result into `nbt`; the network's lowest block hands it to the others.
fn kinetic_step(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, nbt: &mut Compound) {
    if !node_at(&*host, level, pos).is_some() {
        return;
    }
    // a network is solved every SOLVE_EVERY ticks, each block at its own offset
    let time = level.game_time(&*host).unwrap_or(0);
    let offset = i64::from((pos[0].wrapping_mul(31) ^ pos[2]).rem_euclid(SOLVE_EVERY));
    if time.rem_euclid(i64::from(SOLVE_EVERY)) != offset {
        return;
    }
    // a block the lowest block of its network keeps up to date leaves the solving to it
    let stamp = int(nbt, "stamp");
    let fresh = period(time).wrapping_sub(stamp).abs() <= 1;
    if int(nbt, "leader") == 0 && fresh && stamp != 0 {
        return;
    }
    let solution = {
        let h: &dyn Host = &*host;
        solve(|p| node_at(h, level, p), pos)
    };
    let leads = solution.leader() == Some(pos);
    apply(nbt, &solution, pos);
    set_int(nbt, "leader", i32::from(leads));
    set_int(nbt, "stamp", period(time));
    if leads {
        for (at, _) in solution.speeds.iter().filter(|(p, _)| **p != pos) {
            let Some(be) = host.block_entity(level, *at) else { continue };
            let mut patch = Compound::new();
            apply(&mut patch, &solution, *at);
            set_int(&mut patch, "leader", 0);
            set_int(&mut patch, "stamp", period(time));
            let _ = be.merge_data(host, patch);
        }
    }
}

fn progress(nbt: &Compound) -> f64 {
    f64::from(int(nbt, "progress_x1000")) / 1000.0
}

fn set_progress(nbt: &mut Compound, p: f64) {
    set_int(nbt, "progress_x1000", (p * 1000.0).round() as i32);
}

/// Puts `out` into the millstone's output slots (4..9) if it fits there.
fn deposit(host: &dyn Host, items: &mut [Option<ItemStack>], id: &str, count: u32) -> bool {
    let Some(key) = mod_api::Identifier::parse(id).ok().and_then(|i| host.item_key(&i)) else {
        return false;
    };
    let Some(out) = ItemStack::new(key, count) else { return false };
    for slot in MILL_INPUTS..MILL_SLOTS {
        match &mut items[slot] {
            None => {
                items[slot] = Some(out);
                return true;
            }
            Some(held) => {
                let max = host.max_stack_size(held.item()).unwrap_or(64);
                if held.is_stackable_with(&out) && held.count() + count <= max {
                    let _ = held.grow(count);
                    return true;
                }
            }
        }
    }
    false
}

fn tick_millstone(host: &mut dyn ServerHost, speed: f64, nbt: &mut Compound, items: &mut Vec<Option<ItemStack>>) {
    items.resize(MILL_SLOTS, None);
    let found = (0..MILL_INPUTS).find_map(|slot| {
        let id = items[slot].as_ref()?.item().id(&*host);
        mill(&id).map(|(out, n)| (slot, out, n))
    });
    let Some((slot, out, n)) = found else {
        set_progress(nbt, 0.0);
        return;
    };
    if speed == 0.0 {
        return;
    }
    let (p, done) = advance(progress(nbt), speed, MILL_WORK);
    if !done {
        set_progress(nbt, p);
        return;
    }
    if deposit(&*host, items, out, n) {
        items[slot] = items[slot].take().and_then(|i| i.shrink(1));
        set_progress(nbt, 0.0);
    } else {
        // the outputs are full: hold at the end
        set_progress(nbt, MILL_WORK - speed.abs());
    }
}

/// Moves the belt's item one block on when the work is done: onto a belt or depot with a free slot.
fn tick_belt(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: Pos,
    speed: f64,
    nbt: &mut Compound,
    items: &mut Vec<Option<ItemStack>>,
) {
    items.resize(1, None);
    let Some(held) = items[0].clone() else {
        set_progress(nbt, 0.0);
        return;
    };
    let axis = state_at(&*host, level, pos).and_then(|(_, a, ..)| a).unwrap_or(Axis::X);
    let Some(d) = belt_step(axis, speed) else { return };
    let (p, done) = advance(progress(nbt), speed, BELT_WORK);
    if !done {
        set_progress(nbt, p);
        return;
    }
    let to = [pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]];
    let takes = state_at(&*host, level, to)
        .is_some_and(|(id, ..)| matches!(parse_block(&id), Some(Kind::Belt | Kind::Depot)));
    let Some(be) = host.block_entity(level, to).filter(|_| takes) else {
        // nothing to take it: the item waits at the end
        set_progress(nbt, BELT_WORK - speed.abs());
        return;
    };
    if be.item_slot(&*host, 0).ok().flatten().is_some() {
        set_progress(nbt, BELT_WORK - speed.abs());
        return;
    }
    let moved = host
        .transaction(|host, tx| be.set_item_slot(host, tx, 0, Some(held)).map_err(|_| ()).map(|_| ()))
        .is_ok();
    if moved {
        items[0] = None;
        set_progress(nbt, 0.0);
    }
}

/// Mixes what lies in the basin below: takes one of each ingredient, adds the product.
fn tick_mixer(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, speed: f64, nbt: &mut Compound) {
    let below = [pos[0], pos[1] - 1, pos[2]];
    let is_basin = state_at(&*host, level, below).is_some_and(|(id, ..)| parse_block(&id) == Some(Kind::Basin));
    let Some(be) = host.block_entity(level, below).filter(|_| is_basin) else {
        set_progress(nbt, 0.0);
        return;
    };
    let slots: Vec<Option<ItemStack>> = (0..BASIN_SLOTS as u32).map(|i| be.item_slot(&*host, i).ok().flatten()).collect();
    let ids: Vec<String> = slots.iter().flatten().map(|s| s.item().id(&*host).to_string()).collect();
    let Some((take, out, n)) = mix(&ids) else {
        set_progress(nbt, 0.0);
        return;
    };
    if speed == 0.0 {
        return;
    }
    let (p, done) = advance(progress(nbt), speed, MIX_WORK);
    if !done {
        set_progress(nbt, p);
        return;
    }
    set_progress(nbt, 0.0);
    let Some(key) = mod_api::Identifier::parse(out).ok().and_then(|id| host.item_key(&id)) else { return };
    let Some(made) = ItemStack::new(key, n) else { return };
    let mut slots = slots;
    for id in take {
        if let Some(i) = slots.iter().position(|s| s.as_ref().is_some_and(|s| s.item().id(&*host) == id)) {
            slots[i] = slots[i].take().and_then(|s| s.shrink(1));
        }
    }
    // the product goes into the first free slot, or the mix waits
    let Some(free) = slots.iter().position(Option::is_none) else { return };
    slots[free] = Some(made);
    let _ = host.transaction(|host, tx| {
        for (i, s) in slots.into_iter().enumerate() {
            be.set_item_slot(host, tx, i as u32, s).map_err(|_| ())?;
        }
        Ok::<(), ()>(())
    });
}

fn tick_press(host: &mut dyn ServerHost, level: LevelRef, pos: Pos, speed: f64, nbt: &mut Compound) {
    let below = [pos[0], pos[1] - 1, pos[2]];
    let on_depot = state_at(&*host, level, below).is_some_and(|(id, ..)| parse_block(&id) == Some(Kind::Depot));
    let held = if on_depot { host.block_entity(level, below).and_then(|be| be.item_slot(&*host, 0).ok().flatten()) } else { None };
    let result = held.as_ref().and_then(|h| press(&h.item().id(&*host)));
    let (Some(out), Some(held)) = (result, held) else {
        set_progress(nbt, 0.0);
        return;
    };
    if speed == 0.0 {
        return;
    }
    let (p, done) = advance(progress(nbt), speed, PRESS_WORK);
    if !done {
        set_progress(nbt, p);
        return;
    }
    set_progress(nbt, 0.0);
    // the whole stack on the depot is pressed at once
    let Some(key) = mod_api::Identifier::parse(out).ok().and_then(|id| host.item_key(&id)) else { return };
    let Some(sheets) = ItemStack::new(key, held.count()) else { return };
    let Some(be) = host.block_entity(level, below) else { return };
    let _ = host.transaction(|host, tx| {
        be.set_item_slot(host, tx, 0, Some(sheets)).map_err(|_| ())?;
        Ok::<(), ()>(())
    });
}

/// A coarse clock (about ten seconds) for the leader's stamp, so a stamp changes rarely.
fn period(time: i64) -> i32 {
    (time / 200) as i32 + 1
}

/// Writes what a solution says about the block at `pos` into `nbt`.
fn apply(nbt: &mut Compound, solution: &kinetics::Solution, pos: Pos) {
    let speed = solution.speeds.get(&pos).copied().unwrap_or(0.0);
    set_int(nbt, "speed_x1000", (speed * 1000.0).round() as i32);
    set_int(nbt, "capacity", solution.capacity.round() as i32);
    set_int(nbt, "stress", solution.stress.round() as i32);
    set_int(nbt, "jammed", i32::from(solution.conflict));
    set_int(nbt, "overstressed", i32::from(solution.overstressed));
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let ids: Vec<String> = Kind::ALL.iter().map(|k| format!("create:{}", k.name())).collect();
        let names: Vec<&str> = ids.iter().map(String::as_str).collect();
        let built = r.block_entity_type("kinetic").blocks(&names).build();
        let _ = r.block_entity_part(&built, Kinetic);
        let key = r.block_key("create:millstone");
        // hoppers feed the four input slots from above and the sides and empty the outputs below
        let inputs: Vec<usize> = (0..MILL_INPUTS).collect();
        let outputs: Vec<usize> = (MILL_INPUTS..MILL_SLOTS).collect();
        r.block_container(
            &key,
            ModContainer {
                slots: MILL_SLOTS,
                faces: [outputs.clone(), inputs.clone(), inputs.clone(), inputs.clone(), inputs.clone(), inputs.clone()],
                inputs,
                outputs,
                menu: Some("minecraft:generic_9x1".into()),
            },
        );
        let key = r.block_key("create:basin");
        r.block_container(
            &key,
            ModContainer {
                slots: BASIN_SLOTS,
                faces: std::array::from_fn(|_| (0..BASIN_SLOTS).collect()),
                inputs: (0..BASIN_SLOTS).collect(),
                outputs: (0..BASIN_SLOTS).collect(),
                menu: Some("minecraft:generic_9x1".into()),
            },
        );
        let key = r.block_key("create:belt");
        // one item rides a belt; hoppers put onto it and take from it
        r.block_container(
            &key,
            ModContainer {
                slots: 1,
                faces: std::array::from_fn(|_| vec![0]),
                inputs: vec![0],
                outputs: vec![0],
                menu: None,
            },
        );
        let key = r.block_key("create:depot");
        // one stack lies on a depot; hoppers put onto it and take from it
        r.block_container(
            &key,
            ModContainer {
                slots: 1,
                faces: std::array::from_fn(|_| vec![0]),
                inputs: vec![0],
                outputs: vec![0],
                menu: None,
            },
        );
    }
}
