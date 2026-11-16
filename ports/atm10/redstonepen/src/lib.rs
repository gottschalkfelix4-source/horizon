//! Redstone Pen for Horizon: draw redstone tracks on any face of a block with the pen (or the
//! quill, which uses redstone from the inventory), and relays that sit on any face.
//!
//! Written from Redstone Pen's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and what a harness against the NeoForge
//! original still has to confirm, is in README.md.
//!
//! How it works: the track and relays are data-declared blocks (`data/redstonepen/horizon/block/`)
//! with state properties of their own; the parts here give them their redstone behaviour. A track
//! hears a neighbour change, schedules a tick, and on that tick walks its whole network, reads the
//! power each cell gets from outside the network and sets every cell's `power` at once (`wire`).
//! A relay reads the signal behind it, and when its next state differs schedules a tick that
//! writes it (`relays`). The pen draws a track on the clicked face (`pen`).
use std::sync::OnceLock;

use mod_api::{
    BlockKey, BlockPart, BlockSignalEvent, BlockState, BlockUpdateFlags, Direction, EquipmentSlot,
    Flow, Hand, HostBlockStates, HostSide, InteractionResult, IsSignalSourceEvent, ItemKey,
    ItemPart, LevelHost, LevelRef, LevelTicksHost, LevelWriteHost, NeighborChangedEvent,
    OnBlockPlacedByEvent, Registrar, RegistrationContext, ServerHost, StateConstraint,
    StateForPlacementEvent, TickBlockEvent, UpdateShapeEvent, UseOnEvent,
};

pub mod orient;
pub mod pen;
pub mod relays;
pub mod state;
pub mod wire;

use orient::{ALL, bit, look, rotation_towards, sides, step};
use pen::{Cell, Draw};
use relays::{DELAY, RELAYS, Relay};
use state::{GateState, Wire};

/// The track's block id.
pub const TRACK: &str = "redstonepen:track";

/// The track's key, known once registration ran.
static WIRE_KEY: OnceLock<BlockKey> = OnceLock::new();
/// Air's key, to tell an empty cell.
static AIR_KEY: OnceLock<BlockKey> = OnceLock::new();
/// Redstone dust's key: the quill's ink.
static REDSTONE: OnceLock<ItemKey> = OnceLock::new();

fn is_wire(block: BlockKey) -> bool {
    WIRE_KEY.get() == Some(&block)
}

fn pos_of(p: mod_api::BlockPos) -> [i32; 3] {
    [p.x, p.y, p.z]
}

/// Vanilla redstone dust's key: dust reads this mod's signal at full strength, so its signal
/// back loses `wire::DECAY`, or the two would hold each other on.
static DUST_KEY: OnceLock<BlockKey> = OnceLock::new();

fn air(level: &dyn LevelHost) -> Option<BlockState> {
    level.block_state_default(*AIR_KEY.get()?)
}

fn is_air(level: &dyn LevelHost, pos: [i32; 3]) -> bool {
    match level.block_state(pos) {
        Some(s) => AIR_KEY.get() == Some(&s.block()),
        None => true,
    }
}

fn flags() -> BlockUpdateFlags {
    BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS
}

/// A redstone track's behaviour.
struct TrackPart;

/// The power the track at `pos` gets from outside its network: the strongest signal of a
/// neighbour that is not track.
fn wire_input(level: &dyn LevelTicksHost, pos: [i32; 3]) -> u8 {
    orient::ALL
        .into_iter()
        .filter_map(|d| {
            let n = step(pos, d);
            let neighbour = level.block_state(n)?;
            if is_wire(neighbour.block()) {
                return None;
            }
            let signal = level.signal(n, d).clamp(0, 15) as u8;
            if DUST_KEY.get() == Some(&neighbour.block()) {
                return Some(signal.saturating_sub(wire::DECAY));
            }
            Some(signal)
        })
        .max()
        .unwrap_or(0)
}

#[mod_api::horizon_part]
impl BlockPart for TrackPart {
    fn is_signal_source(&self, _: &dyn LevelHost, event: &mut IsSignalSourceEvent) {
        event.source = true;
    }

    /// A track weakly powers every block beside it with the power it carries.
    fn block_signal(&self, level: &dyn LevelHost, event: &mut BlockSignalEvent) {
        if event.direct {
            event.signal = 0;
            return;
        }
        if let Some(w) = state::wire(level, event.state) {
            event.signal = i32::from(w.power);
        }
    }

    fn neighbor_changed(&self, level: &mut dyn LevelTicksHost, event: &mut NeighborChangedEvent) {
        level.schedule_block_tick(pos_of(event.pos), event.state.block(), 1);
    }

    /// Solve the track's whole network and write every power that changed.
    fn tick_block(&self, level: &mut dyn LevelWriteHost, event: &mut TickBlockEvent) -> Flow {
        let start = pos_of(event.pos);
        let net = {
            let read: &dyn LevelHost = &*level;
            wire::gather(start, |p| {
                let s = read.block_state(p)?;
                if !is_wire(s.block()) {
                    return None;
                }
                state::wire(read, s).map(|w| w.faces)
            })
        };
        let inputs: Vec<u8> = net.nodes.iter().map(|p| wire_input(&*level, *p)).collect();
        let power = wire::solve(&inputs, &net.links);
        for (i, p) in net.nodes.iter().enumerate() {
            let Some(s) = level.block_state(*p) else {
                continue;
            };
            let Some(w) = state::wire(&*level, s) else {
                continue;
            };
            if w.power == power[i] {
                continue;
            }
            let next = Wire {
                faces: w.faces,
                power: power[i],
            };
            if let Some(ns) = state::with_wire(&*level, s, next) {
                level.set_block(*p, ns, flags());
            }
        }
        Flow::Cancel
    }

    /// A face whose support went loses its track; a track with no faces left goes.
    fn update_shape(
        &self,
        level: &mut dyn LevelTicksHost,
        event: &mut UpdateShapeEvent,
    ) -> Option<BlockState> {
        let w = state::wire(&*level, event.state)?;
        if w.faces & bit(event.direction) == 0 || !is_air(&*level, pos_of(event.neighbour_pos)) {
            return None;
        }
        let faces = w.faces & !bit(event.direction);
        if faces == 0 {
            return air(&*level);
        }
        state::with_wire(&*level, event.state, Wire { faces, ..w })
    }
}

/// One relay's behaviour.
struct RelayPart(Relay);

fn relay_input(level: &dyn LevelTicksHost, pos: [i32; 3], g: &GateState) -> bool {
    let back = sides(g.attach, g.rotation).back;
    level.signal(step(pos, back), back) > 0
}

#[mod_api::horizon_part]
impl BlockPart for RelayPart {
    fn is_signal_source(&self, _: &dyn LevelHost, event: &mut IsSignalSourceEvent) {
        event.source = true;
    }

    /// A relay weakly powers the block in front of it with full power when on.
    fn block_signal(&self, level: &dyn LevelHost, event: &mut BlockSignalEvent) {
        if event.direct {
            event.signal = 0;
            return;
        }
        if let Some(g) = state::gate(level, event.state) {
            let on = g.memory.powered && event.face == sides(g.attach, g.rotation).output;
            event.signal = if on { 15 } else { 0 };
        }
    }

    fn neighbor_changed(&self, level: &mut dyn LevelTicksHost, event: &mut NeighborChangedEvent) {
        let pos = pos_of(event.pos);
        let Some(g) = state::gate(&*level, event.state) else {
            return;
        };
        if self.0.step(relay_input(&*level, pos, &g), g.memory) != g.memory {
            level.schedule_block_tick(pos, event.state.block(), DELAY);
        }
    }

    fn tick_block(&self, level: &mut dyn LevelWriteHost, event: &mut TickBlockEvent) -> Flow {
        let pos = pos_of(event.pos);
        let Some(g) = state::gate(&*level, event.state) else {
            return Flow::Cancel;
        };
        let on = relay_input(&*level, pos, &g);
        let next = self.0.step(on, g.memory);
        if next != g.memory {
            if let Some(ns) =
                state::with_gate(&*level, event.state, GateState { memory: next, ..g })
            {
                level.set_block(pos, ns, flags());
            }
            // A pulse settles on the next step.
            if self.0.step(on, next) != next {
                level.schedule_block_tick(pos, event.state.block(), DELAY);
            }
        }
        Flow::Cancel
    }

    /// A relay whose support went pops off.
    fn update_shape(
        &self,
        level: &mut dyn LevelTicksHost,
        event: &mut UpdateShapeEvent,
    ) -> Option<BlockState> {
        let g = state::gate(&*level, event.state)?;
        if event.direction != g.attach || !is_air(&*level, pos_of(event.neighbour_pos)) {
            return None;
        }
        air(&*level)
    }

    /// Placed against a face, its output pointing the way the placer looks.
    fn state_for_placement(
        &self,
        level: &dyn LevelHost,
        event: &mut StateForPlacementEvent<'_>,
    ) -> Option<Option<BlockState>> {
        let attach = event.context.face.opposite();
        let pos = pos_of(event.context.pos);
        if is_air(level, step(pos, attach)) {
            return Some(None);
        }
        let rotation = rotation_towards(attach, look(event.context.yaw, event.context.pitch));
        Some(state::with_gate(
            level,
            event.state,
            GateState {
                attach,
                rotation,
                memory: relays::Memory::default(),
            },
        ))
    }
}

/// The relays' items: placing one makes it read its input at once, so an inverted relay placed
/// with nothing behind it turns on.
struct RelayItem;

#[mod_api::horizon_part]
impl ItemPart for RelayItem {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        let Some(level) = event.placer.and_then(|p| p.level(&*host)) else {
            return;
        };
        let _ = level.neighbor_changed(host, event.pos, event.state);
    }
}

/// How a drawing tool pays for a track.
#[derive(Clone, Copy)]
enum Ink {
    /// The pen: one point of its durability per track face.
    Durability,
    /// The quill: one redstone dust from the inventory per track face.
    Redstone,
}

/// The pen and the quill.
struct PenItem(Ink);

fn direction_of(normal: [i32; 3]) -> Option<Direction> {
    ALL.into_iter().find(|d| d.normal() == normal)
}

/// What is in the cell at `pos`, read on the server.
fn cell_at(host: &dyn ServerHost, level: LevelRef, pos: [i32; 3]) -> Cell {
    let Some(s) = level.block_state(host, pos) else {
        return Cell::Other;
    };
    if AIR_KEY.get() == Some(&s.block()) {
        return Cell::Air;
    }
    if !is_wire(s.block()) {
        return Cell::Other;
    }
    let mut faces = 0;
    for d in ALL {
        if level.block_property(host, pos, orient::name(d)).as_deref() == Some("true") {
            faces |= bit(d);
        }
    }
    Cell::Track(faces)
}

/// The track state with `faces` and no power.
fn track_state(host: &dyn ServerHost, faces: u8) -> Option<BlockState> {
    let key = *WIRE_KEY.get()?;
    let mut constraints = vec![StateConstraint::new("power", "0")];
    for d in ALL {
        let on = if faces & bit(d) != 0 { "true" } else { "false" };
        constraints.push(StateConstraint::new(orient::name(d), on));
    }
    BlockState::of(&host.block_states(), key, &constraints).ok()
}

/// Take one redstone dust from the player's inventory; false when there is none.
fn take_redstone(host: &mut dyn ServerHost, player: mod_api::PlayerRef) -> bool {
    let Some(redstone) = REDSTONE.get().copied() else {
        return false;
    };
    let Some(slots) = player.inventory_slots(&*host) else {
        return false;
    };
    let Some((slot, stack)) = slots.into_iter().find(|(_, s)| s.item() == redstone) else {
        return false;
    };
    player
        .set_inventory_slot(host, slot, stack.shrink(1))
        .is_ok()
}

#[mod_api::horizon_part]
impl ItemPart for PenItem {
    /// Draw a track on the clicked face, paid for with ink.
    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        let face = direction_of(event.face)?;
        let player = host.player(event.player)?;
        let level = player.level(&*host)?;
        let target = step(event.pos, face);
        let Draw::Track { pos, faces } = pen::draw(event.pos, face, cell_at(&*host, level, target))
        else {
            return Some(InteractionResult::Fail);
        };
        let state = track_state(&*host, faces)?;
        let free = player.has_infinite_materials(&*host) == Some(true);
        if !free {
            match self.0 {
                Ink::Redstone => {
                    if !take_redstone(host, player) {
                        return Some(InteractionResult::Fail);
                    }
                }
                Ink::Durability => {
                    let living = host.living(player.entity())?;
                    let slot = match event.hand {
                        Hand::Main => EquipmentSlot::MainHand,
                        Hand::Off => EquipmentSlot::OffHand,
                    };
                    living.hurt_and_break(host, slot, 1);
                }
            }
        }
        if level.set_block(host, pos, state, flags()).unwrap_or(false) {
            let _ = level.neighbor_changed(host, pos, state);
        }
        Some(InteractionResult::SuccessServer)
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl mod_api::Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let track = r.block_key(TRACK);
        let _ = WIRE_KEY.set(track);
        let _ = AIR_KEY.set(r.block_key("minecraft:air"));
        let _ = DUST_KEY.set(r.block_key("minecraft:redstone_wire"));
        let _ = REDSTONE.set(r.item_key("minecraft:redstone"));
        r.block_part(track, TrackPart);
        for (relay, id) in RELAYS {
            let key = r.block_key(id);
            r.block_part(key, RelayPart(relay));
            let item = r.item_key(id);
            r.item_part(item, RelayItem);
        }
        let pen = r.item_key("redstonepen:pen");
        r.item_part(pen, PenItem(Ink::Durability));
        let quill = r.item_key("redstonepen:quill");
        r.item_part(quill, PenItem(Ink::Redstone));
    }
}
