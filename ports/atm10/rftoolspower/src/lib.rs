//! RFToolsPower for Horizon: Dimensional Cells, the Coal Generator, the Endergenic Generator
//! with its Pearl Injector, and the Power Monitor.
//!
//! Written from RFToolsPower's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What is ported, and what a check against the NeoForge
//! original still has to confirm, is in README.md.
//!
//! Energy is RF on Horizon's energy capability: each machine's buffer is its block entity's
//! `energy` (`Registrar::block_energy`), and RF moves to and from neighbours through `ENERGY`.
pub mod cell;
pub mod coal;
pub mod endergenic;
pub mod energy;
pub mod monitor;

use std::cell::Cell;

use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, BlockSignalEvent, Compound, Direction, ENERGY,
    EmptyEnergyHandler, EnergyHandler, Hooks, Host, HostTransaction, InteractionResult, IsSignalSourceEvent,
    LevelHost, LevelRef, LevelSide, ModContainer, ModEnergyBuffer, Registrar,
    RegistrationContext, ServerHost, Tag, Transaction, UseWithoutItemEvent,
};

use crate::cell::{SideMode, Sides, Tier, face_of, level_out};
use crate::coal::RedstoneMode;
use crate::endergenic::State;
use crate::monitor::Monitor;

const NS: &str = "rftoolspower";

fn id(path: &str) -> String {
    format!("{NS}:{path}")
}

fn block_at(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<String> {
    level.block(host, pos).map(|k| k.id(host))
}

fn tier_of(block: &str) -> Option<Tier> {
    let path = block.strip_prefix("rftoolspower:")?;
    Tier::ALL.into_iter().find(|t| t.block() == path)
}

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        Some(Tag::Byte(v)) => i32::from(*v),
        _ => 0,
    }
}

/// A rising redstone edge: `powered` now against what the compound remembers, which it then
/// remembers.
fn rising_edge(nbt: &mut Compound, powered: bool) -> bool {
    let was = int(nbt, "powered") != 0;
    nbt.insert("powered".into(), Tag::Byte(i8::from(powered)));
    powered && !was
}

const ALL_FACES: [bool; 6] = [true; 6];

// ---------------------------------------------------------------------------------------------
// Dimensional cells

thread_local! {
    /// Set while a cell's provider asks the game for its plain buffer, so it doesn't ask itself.
    static ASKING_GAME: Cell<bool> = const { Cell::new(false) };
}

/// A cell's buffer as one face reaches it: input faces only take, output faces only give.
struct SidedCell {
    inner: mod_api::Capability<dyn EnergyHandler>,
    mode: SideMode,
}

impl EnergyHandler for SidedCell {
    fn amount(&self, host: &mut dyn ServerHost) -> u32 {
        self.inner.amount(host)
    }
    fn capacity(&self, host: &mut dyn ServerHost) -> u32 {
        self.inner.capacity(host)
    }
    fn insert(&self, host: &mut dyn ServerHost, tx: &Transaction, amount: u32) -> u32 {
        if self.mode.takes() { self.inner.insert(host, tx, amount) } else { 0 }
    }
    fn extract(&self, host: &mut dyn ServerHost, tx: &Transaction, amount: u32) -> u32 {
        if self.mode.gives() { self.inner.extract(host, tx, amount) } else { 0 }
    }
}

fn cell_provider(host: &mut dyn ServerHost, at: mod_api::BlockSide) -> Option<Box<dyn EnergyHandler>> {
    if ASKING_GAME.get() {
        return None;
    }
    // No side: another cell levelling out, which reaches the whole buffer.
    let dir = at.side?;
    let entity = host.block_entity(at.level, at.pos)?;
    let mode = Sides::load(&entity.data(host)?).get(dir);
    if mode == SideMode::None {
        return Some(Box::new(EmptyEnergyHandler));
    }
    ASKING_GAME.set(true);
    let inner = at.level.capability(host, &ENERGY, at.pos, Some(dir));
    ASKING_GAME.set(false);
    Some(Box::new(SidedCell { inner: inner?, mode }))
}

fn tick_cell(host: &mut dyn ServerHost, level: LevelRef, tier: Tier, event: &mut BlockEntityTickEvent<'_>) {
    let pos = event.pos;
    let before = energy::stored(&event.entity.nbt);
    let mut stored = i64::from(before);
    let cap = i64::from(tier.capacity());
    let limit = i64::from(tier.rf_per_tick());
    let sides = Sides::load(&event.entity.nbt);
    for dir in Direction::ALL {
        let at = energy::beside(pos, dir);
        let neighbour_is_cell = block_at(host, level, at).as_deref().and_then(tier_of).is_some();
        if neighbour_is_cell {
            let Some(other) = level.capability(host, &ENERGY, at, None) else {
                continue;
            };
            let (b, cap_b) = (i64::from(other.amount(host)), i64::from(other.capacity(host)));
            let x = level_out(stored, cap, b, cap_b, limit);
            if x > 0 {
                stored -= i64::from(energy::offer(host, level, pos, dir, x as u32).min(x as u32));
            } else if x < 0 {
                let want = (-x).min(cap - stored) as u32;
                stored += i64::from(host
                    .transaction(|host, tx| Ok::<u32, ()>(other.extract(host, tx, want)))
                    .unwrap_or(0));
            }
        } else if sides.get(dir).gives() && stored > 0 {
            stored -= i64::from(energy::offer(host, level, pos, dir, stored.min(limit) as u32));
        }
    }
    if stored == i64::from(before) {
        *event.reply = BlockEntityTickReply::Unchanged;
        return;
    }
    energy::set_stored(&mut event.entity.nbt, stored as i32);
}

// ---------------------------------------------------------------------------------------------
// Coal generator

fn tick_coal(host: &mut dyn ServerHost, level: LevelRef, event: &mut BlockEntityTickEvent<'_>) {
    let pos = event.pos;
    let nbt = &mut event.entity.nbt;
    let mut burning = int(nbt, "burning");
    let mut stored = energy::stored(nbt);
    let allowed = RedstoneMode::load(nbt).allows(level.has_neighbor_signal(host, pos));
    if event.entity.items.len() < 5 {
        event.entity.items.resize(5, None);
    }
    let slot = event.entity.items.iter().position(|s| s.is_some());
    let fuel = slot.and_then(|i| {
        let stack = event.entity.items[i].as_ref()?;
        coal::burn_ticks(&stack.item().id(host), host.item_burn_time(stack.item()))
    });
    let was_lit = burning > 0;
    let step = coal::tick(&mut burning, stored, allowed, fuel);
    if step.consume {
        if let Some(i) = slot {
            let taken = event.entity.items[i].take();
            event.entity.items[i] = taken.and_then(|s| s.shrink(1));
        }
    }
    stored += step.made;
    stored -= energy::push_all(host, level, pos, &energy::ALL_FACES, coal::SEND_PER_TICK as u32, stored as u32) as i32;
    if !step.consume && step.made == 0 && stored == energy::stored(&event.entity.nbt) && burning == int(&event.entity.nbt, "burning") {
        *event.reply = BlockEntityTickReply::Unchanged;
        return;
    }
    let nbt = &mut event.entity.nbt;
    nbt.insert("burning".into(), Tag::Int(burning));
    energy::set_stored(nbt, stored);
    if was_lit != (burning > 0) {
        host.set_block_property(pos, "lit", if burning > 0 { "true" } else { "false" });
    }
}

// ---------------------------------------------------------------------------------------------
// Endergenic generator and pearl injector

/// Sends a pearl of `age` to the endergenic at `to`, arriving after `ticks`. Lost (false) when
/// nothing there takes it.
fn send_pearl(host: &mut dyn ServerHost, level: LevelRef, to: [i32; 3], age: i32, ticks: i32) -> bool {
    if block_at(host, level, to).as_deref() != Some("rftoolspower:endergenic") {
        return false;
    }
    let Some(target) = host.block_entity(level, to) else {
        return false;
    };
    let Some(data) = target.data(host) else {
        return false;
    };
    let mut incoming = endergenic::load_incoming(&data);
    incoming.push((age, ticks));
    let mut patch = Compound::new();
    endergenic::save_incoming(&mut patch, &incoming);
    target.merge_data(host, patch).is_ok()
}

fn tick_endergenic(host: &mut dyn ServerHost, level: LevelRef, event: &mut BlockEntityTickEvent<'_>) {
    let pos = event.pos;
    let powered = level.has_neighbor_signal(host, pos);
    let nbt = &mut event.entity.nbt;
    let mut state = State::load(nbt);
    let mut stored = energy::stored(nbt);
    let mut fired = None;
    if rising_edge(nbt, powered) {
        fired = endergenic::pulse(&mut state).fired;
    }
    let mut incoming = endergenic::load_incoming(nbt);
    let mut arrived = Vec::new();
    incoming.retain_mut(|(age, t)| {
        *t -= 1;
        if *t <= 0 {
            arrived.push(*age);
            false
        } else {
            true
        }
    });
    for age in arrived {
        let o = endergenic::arrive(&mut state, age);
        stored = (stored + o.gained).min(endergenic::CAPACITY);
    }
    let o = endergenic::tick(&mut state, stored);
    stored -= o.spent;
    state.save(nbt);
    endergenic::save_incoming(nbt, &incoming);
    energy::set_stored(nbt, stored);
    if let Some(age) = fired {
        let dest = match nbt.get(&"dest".into()) {
            Some(Tag::IntArray(d)) if d.len() == 3 => Some([d[0], d[1], d[2]]),
            _ => None,
        };
        if let Some(to) = dest {
            send_pearl(host, level, to, age, endergenic::flight_ticks(pos, to));
        }
    }
}

fn tick_injector(host: &mut dyn ServerHost, level: LevelRef, event: &mut BlockEntityTickEvent<'_>) {
    let pos = event.pos;
    let powered = level.has_neighbor_signal(host, pos);
    if !rising_edge(&mut event.entity.nbt, powered) {
        return;
    }
    let Some(slot) = event.entity.items.iter().position(|s| {
        s.as_ref().is_some_and(|s| s.item().id(host) == "minecraft:ender_pearl")
    }) else {
        return;
    };
    let target = Direction::ALL
        .into_iter()
        .map(|d| energy::beside(pos, d))
        .find(|p| block_at(host, level, *p).as_deref() == Some("rftoolspower:endergenic"));
    if let Some(to) = target {
        if send_pearl(host, level, to, 0, 1) {
            let taken = event.entity.items[slot].take();
            event.entity.items[slot] = taken.and_then(|s| s.shrink(1));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Power monitor

fn tick_monitor(host: &mut dyn ServerHost, level: LevelRef, event: &mut BlockEntityTickEvent<'_>) {
    let timer = int(&event.entity.nbt, "timer") + 1;
    event.entity.nbt.insert("timer".into(), Tag::Int(timer % 10));
    if timer < 10 {
        return;
    }
    let pos = event.pos;
    let reading = Direction::ALL.into_iter().find_map(|d| energy::read(host, level, pos, d));
    let percent = reading.map_or(0, |(a, c)| Monitor::percent(i64::from(a), i64::from(c)));
    let m = Monitor::load(&event.entity.nbt);
    let powered = reading.is_some() && m.powered(percent);
    let lvl = Monitor::level(percent).to_string();
    if level.block_property(host, pos, "powered").as_deref() != Some(if powered { "true" } else { "false" }) {
        host.set_block_property(pos, "powered", if powered { "true" } else { "false" });
    }
    if level.block_property(host, pos, "level").as_deref() != Some(lvl.as_str()) {
        host.set_block_property(pos, "level", &lvl);
    }
}

// ---------------------------------------------------------------------------------------------
// Parts

struct Machines;

#[mod_api::horizon_part]
impl BlockPart for Machines {
    /// A click on a cell's face cycles that face: none, input, output.
    fn use_without_item(&self, host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
        let level = host.level(host.dimension())?;
        let pos = event.click.pos;
        tier_of(&block_at(host, level, pos)?)?;
        let face = face_of(event.click.hit.face)?;
        let entity = host.block_entity(level, pos)?;
        let mut sides = Sides::load(&entity.data(host)?);
        sides.cycle(face);
        let mut patch = Compound::new();
        sides.save(&mut patch);
        let _ = entity.merge_data(host, patch);
        host.block_entity_sync(pos);
        Some(InteractionResult::Success)
    }

    fn is_signal_source(&self, level: &dyn LevelHost, event: &mut IsSignalSourceEvent) {
        if level.state_block(event.state) == Some("rftoolspower:power_monitor") {
            event.source = true;
        }
    }

    fn block_signal(&self, level: &dyn LevelHost, event: &mut BlockSignalEvent) {
        if level.state_block(event.state) != Some("rftoolspower:power_monitor") {
            return;
        }
        if level.state_string(event.state).is_some_and(|s| s.contains("powered=true")) {
            event.signal = 15;
        }
    }
}

const TICKING: [&str; 7] = ["cell1", "cell2", "cell3", "coalgenerator", "endergenic", "pearl_injector", "power_monitor"];

fn ours(block: &str) -> Option<&str> {
    let path = block.strip_prefix("rftoolspower:")?;
    TICKING.contains(&path).then_some(path)
}

#[mod_api::horizon_part]
impl BlockEntityPart for Machines {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        let path = ours(event.block)?;
        let mut nbt = Compound::new();
        match path {
            "power_monitor" => Monitor::default().save(&mut nbt),
            "pearl_injector" => {}
            "endergenic" => {
                State::Idle.save(&mut nbt);
                energy::set_stored(&mut nbt, 0);
            }
            _ => energy::set_stored(&mut nbt, 0),
        }
        Some(nbt)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if ours(event.block).is_some() {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        let Some(block) = block_at(host, level, event.pos) else {
            return;
        };
        if let Some(tier) = tier_of(&block) {
            return tick_cell(host, level, tier, event);
        }
        match ours(&block) {
            Some("coalgenerator") => tick_coal(host, level, event),
            Some("endergenic") => tick_endergenic(host, level, event),
            Some("pearl_injector") => tick_injector(host, level, event),
            Some("power_monitor") => tick_monitor(host, level, event),
            _ => {}
        }
    }
}

#[derive(Default)]
struct Mod;

fn hopper_like(slots: usize) -> ModContainer {
    let all: Vec<usize> = (0..slots).collect();
    ModContainer {
        slots,
        faces: std::array::from_fn(|_| all.clone()),
        inputs: all,
        outputs: Vec::new(),
        menu: Some("minecraft:hopper".into()),
        slot_limit: 0,
    }
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for tier in Tier::ALL {
            let key = r.block_key(&id(tier.block()));
            let rate = tier.rf_per_tick();
            r.block_energy(&key, ModEnergyBuffer {
                capacity: tier.capacity(),
                max_insert: rate,
                max_extract: rate,
                insert_faces: ALL_FACES,
                extract_faces: ALL_FACES,
            });
        }
        let coal = r.block_key(&id("coalgenerator"));
        r.block_energy(&coal, ModEnergyBuffer {
            capacity: coal::CAPACITY,
            max_insert: 0,
            max_extract: coal::SEND_PER_TICK,
            insert_faces: [false; 6],
            extract_faces: ALL_FACES,
        });
        r.block_container(&coal, hopper_like(5));
        let ender = r.block_key(&id("endergenic"));
        r.block_energy(&ender, ModEnergyBuffer {
            capacity: endergenic::CAPACITY,
            max_insert: 50_000,
            max_extract: 50_000,
            insert_faces: ALL_FACES,
            extract_faces: ALL_FACES,
        });
        let injector = r.block_key(&id("pearl_injector"));
        r.block_container(&injector, hopper_like(5));

        let cells: Vec<String> = Tier::ALL.iter().map(|t| id(t.block())).collect();
        let cell_names: Vec<&str> = cells.iter().map(String::as_str).collect();
        let cell_kind = r.block_entity_type("cell").blocks(&cell_names).build();
        let _ = r.block_entity_part(&cell_kind, Machines);
        r.block_entity_capability_before_game(&ENERGY, &cell_kind, cell_provider);
        for path in ["coalgenerator", "endergenic", "pearl_injector", "power_monitor"] {
            let block = id(path);
            let kind = r.block_entity_type(path).blocks(&[block.as_str()]).build();
            let _ = r.block_entity_part(&kind, Machines);
        }
    }
}
