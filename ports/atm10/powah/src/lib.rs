//! Powah for Horizon: energy cells, cables, four generators in seven tiers, the reactor, the
//! energizing orb and its rods, and ender cells.
//!
//! Written from Powah's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and which numbers a check against the
//! NeoForge original still has to confirm, is in README.md.
//!
//! Every block is `powah:<kind>_<tier>` (the orb has no tier) with one block entity type per kind.
//! Energy lives in the block entity's saved `energy` (the game's `ModEnergyBuffer`), so cables,
//! pipes and other mods' machines reach it through `ENERGY`. Generators push into their
//! neighbours; cables form a network whose lowest cable moves the energy for all of them.
use std::collections::{BTreeSet, VecDeque};

use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Compound, Direction, ENERGY, Holder, Hooks, Host,
    HostTransaction, Identifier, InteractionResult, ItemKey, ItemPart, ItemStack, LevelRef,
    LevelSide, ModContainer, ModEnergyBuffer, ModFluidTanks, OnBlockPlacedByEvent, Registrar,
    RegistrationContext, ServerHost, Tag, UseItemOnEvent, UseWithoutItemEvent,
};

pub mod rules;
use rules::*;

// ---------------------------------------------------------------------------------------------
// Saved data
// ---------------------------------------------------------------------------------------------

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

fn energy(nbt: &Compound) -> i32 {
    int(nbt, "energy").max(0)
}

/// The first tank of a `ModFluidTanks` block, as the game saves it: (fluid id, amount).
fn tank(nbt: &Compound) -> (Option<String>, i32) {
    let Some(Tag::List(list)) = nbt.get(&"fluids".into()) else {
        return (None, 0);
    };
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

fn set_tank(nbt: &mut Compound, fluid: Option<&str>, amount: i32) {
    let mut t = Compound::new();
    let fluid = if amount > 0 { fluid.unwrap_or("minecraft:empty") } else { "minecraft:empty" };
    t.insert("Fluid".into(), Tag::String(fluid.into()));
    t.insert("Amount".into(), Tag::Int(amount.max(0)));
    nbt.insert("fluids".into(), Tag::List(vec![Tag::Compound(t)]));
}

fn offset(pos: [i32; 3], dir: Direction) -> [i32; 3] {
    let n = dir.normal();
    [pos[0] + n[0], pos[1] + n[1], pos[2] + n[2]]
}

fn block_id(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<String> {
    level.block(host, pos).map(|k| k.id(host))
}

fn item_id(host: &dyn Host, stack: &ItemStack) -> String {
    stack.item().id(host)
}

fn item_key(host: &dyn Host, id: &str) -> Option<ItemKey> {
    host.item_key(&Identifier::parse(id).ok()?)
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

// ---------------------------------------------------------------------------------------------
// The block entity part
// ---------------------------------------------------------------------------------------------

/// One part for every Powah block entity; the block's id says which kind it is.
struct Machine;

#[mod_api::horizon_part]
impl BlockPart for Machine {
    /// An ender cell, used with an empty hand while sneaking, moves to the next channel.
    fn use_without_item(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseWithoutItemEvent<'_>,
    ) -> Option<InteractionResult> {
        let level = host.level(host.dimension())?;
        let pos = event.click.pos;
        let id = block_id(&*host, level, pos)?;
        let (Kind::EnderCell, _) = parse_block(&id)? else {
            return None;
        };
        let player = host.player(event.click.player)?;
        if !player.entity().is_shift_key_down(&*host).unwrap_or(false) {
            return None;
        }
        let entity = host.block_entity(level, pos)?;
        let data = entity.data(&*host)?;
        let next = (int(&data, "channel") + 1).rem_euclid(ENDER_CHANNELS);
        let mut patch = Compound::new();
        set_int(&mut patch, "channel", next);
        let _ = entity.merge_data(host, patch);
        let line = mod_api::Text::translate(
            "message.powah.ender_cell.channel",
            vec![mod_api::Text::literal((next + 1).to_string())],
        );
        let _ = player.action_bar(host, line);
        Some(InteractionResult::SuccessServer)
    }

    /// A magmator or thermo generator takes a bucket of lava or water into its tank.
    fn use_item_on(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseItemOnEvent<'_>,
    ) -> Option<InteractionResult> {
        let level = host.level(host.dimension())?;
        let id = block_id(&*host, level, event.click.pos)?;
        let (kind, _) = parse_block(&id)?;
        if !matches!(kind, Kind::Magmator | Kind::Thermo | Kind::Reactor) {
            return None;
        }
        let player = host.player(event.click.player)?;
        let moved = mod_api::fluid_util::interact_with_fluid_handler(
            host,
            player,
            event.click.hand,
            level,
            event.click.pos,
            Direction::ALL.into_iter().find(|d| d.normal() == event.click.hit.face),
        );
        moved.then_some(InteractionResult::SuccessServer)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for Machine {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        let (kind, _) = parse_block(event.block)?;
        let mut nbt = Compound::new();
        set_int(&mut nbt, "energy", 0);
        if kind.has_tank() {
            set_tank(&mut nbt, None, 0);
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
        let Some(id) = block_id(&*host, level, pos) else {
            return;
        };
        let Some((kind, tier)) = parse_block(&id) else {
            return;
        };
        let mut nbt_local = event.entity.nbt.clone();
        let mut items_local = event.entity.items.clone();
        let nbt = &mut nbt_local;
        match (kind, tier) {
            (Kind::Cell, Some(t)) => tick_cell(host, level, pos, t, nbt),
            (Kind::Cable, Some(t)) => tick_cable(host, level, pos, t, nbt),
            (Kind::Solar, Some(t)) => {
                let day = level.get_sky_darken(&*host).is_some_and(|d| d < 4);
                let above = [pos[0], pos[1] + 1, pos[2]];
                let sky = level.can_see_sky(&*host, above).unwrap_or(false);
                let rain = level.is_raining_at(&*host, above).unwrap_or(false);
                let made = solar_output(t, day, sky, rain);
                generate(host, level, pos, kind, t, nbt, made);
            }
            (Kind::Furnator, Some(t)) => {
                let items = &mut items_local;
                if items.len() < FURNATOR_SLOTS {
                    items.resize(FURNATOR_SLOTS, None);
                }
                let mut burn = Burn::load(nbt);
                let room = generator_capacity(Kind::Furnator, t) - energy(nbt);
                let made = burn.tick(t.furnator(), room, || {
                    let slot = items.iter().position(|s| {
                        s.as_ref().is_some_and(|s| host.item_burn_time(s.item()).unwrap_or(0) > 0)
                    })?;
                    let stack = items[slot].take()?;
                    let ticks = host.item_burn_time(stack.item()).unwrap_or(0);
                    let remainder = host.item_crafting_remainder(&stack);
                    items[slot] = stack.shrink(1).or(remainder);
                    Some(furnator_fuel_energy(ticks))
                });
                burn.save(nbt);
                generate(host, level, pos, kind, t, nbt, made);
            }
            (Kind::Magmator, Some(t)) => {
                let (fluid, amount) = tank(nbt);
                let lava = if fluid.as_deref() == Some("minecraft:lava") { amount } else { 0 };
                let mut burn = Burn::load(nbt);
                let room = generator_capacity(Kind::Magmator, t) - energy(nbt);
                let mut used = 0;
                let made = burn.tick(t.magmator(), room, || {
                    (lava >= MAGMATOR_MB_PER_DRAW).then(|| {
                        used = MAGMATOR_MB_PER_DRAW;
                        MAGMATOR_MB_PER_DRAW as i64 * MAGMATOR_FE_PER_MB
                    })
                });
                if used > 0 {
                    set_tank(nbt, fluid.as_deref(), lava - used);
                }
                burn.save(nbt);
                generate(host, level, pos, kind, t, nbt, made);
            }
            (Kind::Thermo, Some(t)) => {
                let below = [pos[0], pos[1] - 1, pos[2]];
                let heat = heat_source(&*host, level, below);
                let (fluid, amount) = tank(nbt);
                let coolant = fluid.as_deref().and_then(coolant_of);
                let room = generator_capacity(Kind::Thermo, t) - energy(nbt);
                let step = thermo_step(t, heat, coolant.map(|c| (c, amount)), room);
                if step.coolant_used > 0 {
                    set_tank(nbt, fluid.as_deref(), amount - step.coolant_used);
                }
                generate(host, level, pos, kind, t, nbt, step.energy);
            }
            (Kind::Reactor, Some(t)) => {
                let items = &mut items_local;
                if items.len() < REACTOR_SLOTS {
                    items.resize(REACTOR_SLOTS, None);
                }
                let mut core = Reactor::load(nbt);
                // Feed: uraninite into fuel, coal or charcoal into carbon, redstone into redstone.
                for slot in 0..REACTOR_SLOTS {
                    let Some(stack) = items[slot].clone() else { continue };
                    let id = item_id(&*host, &stack);
                    if core.feed(&id) {
                        items[slot] = stack.shrink(1);
                    }
                }
                let (fluid, amount) = tank(nbt);
                let coolant = fluid.as_deref().and_then(coolant_of);
                let powered = level.has_neighbor_signal(&*host, pos);
                let room = generator_capacity(Kind::Reactor, t) - energy(nbt);
                let step = core.tick(t, !powered, coolant.map(|c| (c, amount)), room);
                if step.coolant_used > 0 {
                    set_tank(nbt, fluid.as_deref(), amount - step.coolant_used);
                }
                core.save(nbt);
                generate(host, level, pos, kind, t, nbt, step.energy);
            }
            (Kind::Rod, Some(_)) => {} // filled by cables; the orb draws from it
            (Kind::EnderCell, Some(t)) => tick_ender(host, pos, t, nbt),
            (Kind::Orb, None) => {
                let items = &mut items_local;
                if items.len() < ORB_SLOTS {
                    items.resize(ORB_SLOTS, None);
                }
                tick_orb(host, level, pos, nbt, items);
            }
            _ => {}
        }
        if nbt_local == event.entity.nbt && items_local == event.entity.items {
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        }
        event.entity.nbt = nbt_local;
        event.entity.items = items_local;
    }
}

/// Adds what a generator made to its buffer, pushes out what it can, and shows it lit while it
/// makes energy.
fn generate(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], kind: Kind, t: Tier, nbt: &mut Compound, made: i64) {
    let cap = generator_capacity(kind, t);
    let mut e = (i64::from(energy(nbt)) + made).min(i64::from(cap)) as i32;
    e -= push_to_neighbours(host, level, pos, e, generator_output(kind, t), |_, _| true);
    set_int(nbt, "energy", e);
    let now = i32::from(made > 0);
    if int(nbt, "lit") != now {
        set_int(nbt, "lit", now);
        let _ = host.set_block_property(pos, "lit", if now == 1 { "true" } else { "false" });
    }
}

fn is_powah(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> bool {
    block_id(host, level, pos).is_some_and(|id| parse_block(&id).is_some())
}

/// A cell gives to neighbours that are not Powah's (its cables pull from it themselves).
fn tick_cell(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], t: Tier, nbt: &mut Compound) {
    let e = energy(nbt);
    if e == 0 {
        return;
    }
    let given = push_to_neighbours(host, level, pos, e, t.cell_transfer(), |h, n| !is_powah(h, level, n));
    set_int(nbt, "energy", e - given);
}

/// An endpoint of a cable network: a block with `ENERGY` next to a cable.
struct Endpoint {
    pos: [i32; 3],
    side: Direction,
    limit: u32,
    store: bool,
}

/// The cable network's lowest cable moves the energy of all of them: it gathers what generators
/// pushed into every cable, feeds the machines on the network, tops up from the cells when that is
/// not enough, and stores what is left in the cells.
fn tick_cable(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], t: Tier, nbt: &mut Compound) {
    let cable_tier = |h: &dyn Host, p: [i32; 3]| {
        block_id(h, level, p).and_then(|id| match parse_block(&id) {
            Some((Kind::Cable, Some(t))) => Some(t),
            _ => None,
        })
    };
    // The network, breadth first, bounded.
    let mut seen = BTreeSet::from([pos]);
    let mut queue = VecDeque::from([pos]);
    let mut cables = Vec::new();
    let mut ends = Vec::new();
    while let Some(c) = queue.pop_front() {
        let Some(tier) = cable_tier(&*host, c) else { continue };
        cables.push(c);
        for dir in Direction::ALL {
            let n = offset(c, dir);
            if seen.contains(&n) {
                continue;
            }
            if cable_tier(&*host, n).is_some() {
                if seen.len() < MAX_NETWORK {
                    seen.insert(n);
                    queue.push_back(n);
                }
                continue;
            }
            let store = block_id(&*host, level, n).is_some_and(|id| {
                matches!(parse_block(&id), Some((Kind::Cell | Kind::EnderCell, _)))
            });
            let producer = block_id(&*host, level, n).is_some_and(|id| {
                matches!(parse_block(&id), Some((k, _)) if k.is_generator())
            });
            if !producer {
                ends.push(Endpoint { pos: n, side: dir.opposite(), limit: tier.cable_transfer() as u32, store });
            }
        }
    }
    // Only the lowest cable runs the network.
    if cables.iter().min() != Some(&pos) {
        return;
    }
    let mut pool = energy(nbt) as u32;
    for &c in cables.iter().filter(|c| **c != pos) {
        if let Some(cap) = level.capability(host, &ENERGY, c, None) {
            let amount = cap.amount(host);
            pool += host
                .transaction(|host, tx| Ok::<u32, ()>(cap.extract(host, tx, amount)))
                .unwrap_or(0);
        }
    }
    // Machines first, from the pool, then from the cells.
    let caps: Vec<_> = ends
        .iter()
        .map(|e| level.capability(host, &ENERGY, e.pos, Some(e.side)))
        .collect();
    for (end, cap) in ends.iter().zip(&caps) {
        let (false, Some(cap)) = (end.store, cap) else { continue };
        let mut want = end.limit;
        let given = host
            .transaction(|host, tx| Ok::<u32, ()>(cap.insert(host, tx, pool.min(want))))
            .unwrap_or(0);
        pool -= given;
        want -= given;
        for (cell, cell_cap) in ends.iter().zip(&caps) {
            if want == 0 {
                break;
            }
            let (true, Some(cell_cap)) = (cell.store, cell_cap) else { continue };
            let moved = host
                .transaction(|host, tx| mod_api::move_energy(host, cell_cap, cap, want.min(cell.limit), tx))
                .unwrap_or(0);
            want -= moved.min(want);
        }
    }
    for (end, cap) in ends.iter().zip(&caps) {
        let (true, Some(cap)) = (end.store, cap) else { continue };
        if pool == 0 {
            break;
        }
        pool -= host
            .transaction(|host, tx| Ok::<u32, ()>(cap.insert(host, tx, pool.min(end.limit))))
            .unwrap_or(0);
    }
    // What is left goes back into the cables, this one first.
    let own = pool.min(t.cable_transfer() as u32);
    pool -= own;
    for &c in cables.iter().filter(|c| **c != pos) {
        if pool == 0 {
            break;
        }
        if let Some(cap) = level.capability(host, &ENERGY, c, None) {
            pool -= host
                .transaction(|host, tx| Ok::<u32, ()>(cap.insert(host, tx, pool)))
                .unwrap_or(0);
        }
    }
    set_int(nbt, "energy", own as i32);
}

/// Shared ender channels, per owner, on this level.
static ENDER: mod_api::AttachmentType<Vec<EnderChannel>> = mod_api::AttachmentType {
    namespace: "powah",
    name: "ender_channels",
    default: Vec::new,
    codec: Some(mod_api::AttachmentCodec {
        encode: |channels| Ok(encode_channels(channels)),
        decode: |data| decode_channels(data).ok_or(mod_api::AttachmentError::Decode),
    }),
    copy_on_death: false,
    sync: false,
};

fn tick_ender(host: &mut dyn ServerHost, pos: [i32; 3], t: Tier, nbt: &mut Compound) {
    let owner = long(nbt, "owner_hi") as u128 * (1u128 << 64) + long(nbt, "owner_lo") as u64 as u128;
    let channel = int(nbt, "channel").rem_euclid(ENDER_CHANNELS);
    let Ok(mut channels) = ENDER.get(host.attachments(), Holder::Level) else {
        return;
    };
    let mut e = energy(nbt);
    exchange_with_channel(&mut channels, owner, channel, &mut e, t.ender_capacity());
    let _ = ENDER.set(host.attachments(), Holder::Level, channels);
    set_int(nbt, "energy", e);
    let _ = pos;
}

/// The orb: finds a recipe in its inputs and fills it from the rods around it.
fn tick_orb(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    nbt: &mut Compound,
    items: &mut [Option<ItemStack>],
) {
    let inputs: Vec<(String, u32)> = items[..ORB_OUTPUT]
        .iter()
        .flatten()
        .map(|s| (item_id(&*host, s), s.count()))
        .collect();
    let Some(recipe) = find_energizing(&inputs) else {
        if long(nbt, "progress") != 0 {
            set_long(nbt, "progress", 0);
        }
        return;
    };
    // The output must fit.
    let Some(out_key) = item_key(&*host, recipe.output) else { return };
    let fits = match &items[ORB_OUTPUT] {
        None => true,
        Some(held) => held.item() == out_key && held.count() + recipe.count <= 64,
    };
    if !fits {
        return;
    }
    // Draw from every rod in range.
    let mut progress = long(nbt, "progress");
    let need = recipe.energy - progress;
    let mut drawn = 0i64;
    for p in rods_around(pos) {
        if drawn >= need {
            break;
        }
        let Some(id) = block_id(&*host, level, p) else { continue };
        let Some((Kind::Rod, Some(t))) = parse_block(&id) else { continue };
        let Some(cap) = level.capability(host, &ENERGY, p, None) else { continue };
        let want = (need - drawn).min(i64::from(t.rod_transfer())) as u32;
        drawn += i64::from(
            host.transaction(|host, tx| Ok::<u32, ()>(cap.extract(host, tx, want)))
                .unwrap_or(0),
        );
    }
    progress += drawn;
    if progress >= recipe.energy {
        progress = 0;
        consume_ingredients(&*host, items, recipe);
        items[ORB_OUTPUT] = match items[ORB_OUTPUT].take() {
            None => ItemStack::new(out_key, recipe.count),
            Some(mut held) => {
                let _ = held.grow(recipe.count);
                Some(held)
            }
        };
    }
    set_long(nbt, "progress", progress);
}

fn consume_ingredients(host: &dyn Host, items: &mut [Option<ItemStack>], recipe: &Energizing) {
    for (id, count) in recipe.inputs {
        let mut left = *count;
        for slot in items[..ORB_OUTPUT].iter_mut() {
            if left == 0 {
                break;
            }
            let Some(stack) = slot.take() else { continue };
            if stack.item().id(host) != *id {
                *slot = Some(stack);
                continue;
            }
            let take = left.min(stack.count());
            left -= take;
            *slot = stack.shrink(take);
        }
    }
}

/// The heat of the block at `pos` (a lava source, magma, a blazing crystal block).
fn heat_source(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> i32 {
    if let Some(fluid) = level.fluid_state(host, pos) {
        if fluid.source && Some(fluid.fluid) == host.fluid_key("minecraft:lava") {
            return heat_of("minecraft:lava");
        }
    }
    block_id(host, level, pos).map_or(0, |id| heat_of(&id))
}

/// Records who placed an ender cell, so its channels are theirs.
struct OwnedItem;

#[mod_api::horizon_part]
impl ItemPart for OwnedItem {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        let Some(placer) = event.placer else { return };
        let Some(uuid) = placer.entity().uuid(host) else { return };
        let Some(level) = host.level(host.dimension()) else { return };
        let Some(entity) = host.block_entity(level, event.pos) else { return };
        let mut patch = Compound::new();
        set_long(&mut patch, "owner_hi", (uuid >> 64) as i64);
        set_long(&mut patch, "owner_lo", uuid as u64 as i64);
        let _ = entity.merge_data(host, patch);
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for kind in Kind::ALL {
            let ids = kind.block_ids();
            let names: Vec<&str> = ids.iter().map(String::as_str).collect();
            for (id, tier) in ids.iter().zip(kind.tiers()) {
                let key = r.block_key(id);
                if let Some(buffer) = buffer_of(kind, tier) {
                    r.block_energy(&key, buffer);
                }
                if kind.has_tank() {
                    let mut tanks = ModFluidTanks::default();
                    tanks.tanks = vec![TANK_MB];
                    tanks.faces = std::array::from_fn(|_| vec![0]);
                    tanks.inputs = vec![0];
                    r.block_fluids(&key, tanks);
                }
                if let Some(container) = container_of(kind) {
                    r.block_container(&key, container);
                }
                if kind == Kind::EnderCell {
                    let item = r.item_key(id);
                    r.item_part(item, OwnedItem);
                }
            }
            let built = r.block_entity_type(kind.entity_type()).blocks(&names).build();
            let _ = r.block_entity_part(&built, Machine);
        }
    }
}

/// The game's buffer for a block: what goes in and out, and through which faces.
fn buffer_of(kind: Kind, tier: Option<Tier>) -> Option<ModEnergyBuffer> {
    let all = [true; 6];
    let none = [false; 6];
    let (capacity, max_insert, max_extract, ins, out) = match (kind, tier?) {
        (Kind::Cell, t) => (t.cell_capacity(), t.cell_transfer(), t.cell_transfer(), all, all),
        (Kind::EnderCell, t) => (t.ender_capacity(), t.cell_transfer(), t.cell_transfer(), all, all),
        (Kind::Cable, t) => (t.cable_transfer(), t.cable_transfer(), t.cable_transfer(), all, all),
        (Kind::Rod, t) => (t.rod_transfer() * 20, t.rod_transfer(), t.rod_transfer(), all, all),
        (k, t) if k.is_generator() => {
            (generator_capacity(k, t), 0, generator_output(k, t), none, all)
        }
        _ => return None,
    };
    Some(ModEnergyBuffer { capacity, max_insert, max_extract, insert_faces: ins, extract_faces: out })
}

/// The slots a block keeps, and the vanilla menu that shows them.
fn container_of(kind: Kind) -> Option<ModContainer> {
    let (slots, inputs, outputs, menu) = match kind {
        Kind::Furnator => (FURNATOR_SLOTS, (0..FURNATOR_SLOTS).collect(), vec![], "minecraft:hopper"),
        Kind::Reactor => (REACTOR_SLOTS, (0..REACTOR_SLOTS).collect(), vec![], "minecraft:hopper"),
        Kind::Orb => (ORB_SLOTS, (0..ORB_OUTPUT).collect(), vec![ORB_OUTPUT], "minecraft:generic_3x3"),
        _ => return None,
    };
    let every: Vec<usize> = (0..slots).collect();
    Some(ModContainer {
        slots,
        faces: std::array::from_fn(|_| every.clone()),
        inputs,
        outputs,
        menu: Some(menu.into()),
        slot_limit: 0,
    })
}
