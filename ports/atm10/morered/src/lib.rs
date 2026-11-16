//! More Red for Horizon: red alloy wire that sits on any face of a block and carries a redstone
//! signal, and logic gates that sit on any face too.
//!
//! Written from More Red's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and what a harness against the NeoForge
//! original still has to confirm, is in README.md.
//!
//! How it works: each block is a data-declared block (`data/morered/horizon/block/`) with state
//! properties of its own; the parts here give it its redstone behaviour. A wire hears a
//! neighbour change, schedules a tick, and on that tick walks its whole network, reads the power
//! each wire block gets from outside the network and sets every wire's `power` at once (`wire`).
//! A gate reads its three inputs, and when its next state differs schedules a tick one redstone
//! tick later that writes it (`gates`).
use std::sync::OnceLock;

use mod_api::{
    BlockKey, BlockPart, BlockSignalEvent, BlockState, BlockUpdateFlags, Direction, Flow, Hooks,
    IsSignalSourceEvent, ItemPart, LevelHost, LevelTicksHost, LevelWriteHost, NeighborChangedEvent,
    OnBlockPlacedByEvent, Registrar, RegistrationContext, ServerHost, StateForPlacementEvent,
    TickBlockEvent, UpdateShapeEvent,
};

pub mod gates;
pub mod orient;
pub mod state;
pub mod wire;

use gates::{DELAY, GATES, Gate, Inputs};
use orient::{bit, look, rotation_towards, sides, step};
use state::{GateState, Wire};

/// The wire's block id.
pub const WIRE: &str = "morered:red_alloy_wire";

/// The wire's key, known once registration ran.
static WIRE_KEY: OnceLock<BlockKey> = OnceLock::new();
/// Air's key, to tell an empty cell.
static AIR_KEY: OnceLock<BlockKey> = OnceLock::new();

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

/// The red alloy wire's behaviour.
struct WirePart;

/// The power the wire at `pos` gets from outside its network: the strongest signal of a
/// neighbour that is not wire.
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
impl BlockPart for WirePart {
    fn is_signal_source(&self, _: &dyn LevelHost, event: &mut IsSignalSourceEvent) {
        event.source = true;
    }

    /// A wire weakly powers every block beside it with the power it carries.
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

    /// Solve the wire's whole network and write every power that changed.
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

    /// A face whose support went loses its wire; a wire with no faces left goes.
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

    /// Placed against a face: the wire lies on that face.
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
        Some(state::with_wire(
            level,
            event.state,
            Wire {
                faces: bit(attach),
                power: 0,
            },
        ))
    }
}

/// One gate's behaviour.
struct GatePart(Gate);

fn gate_inputs(level: &dyn LevelTicksHost, pos: [i32; 3], g: &GateState) -> Inputs {
    let s = sides(g.attach, g.rotation);
    let on = |d: Direction| level.signal(step(pos, d), d) > 0;
    Inputs {
        left: on(s.left),
        back: on(s.back),
        right: on(s.right),
    }
}

#[mod_api::horizon_part]
impl BlockPart for GatePart {
    fn is_signal_source(&self, _: &dyn LevelHost, event: &mut IsSignalSourceEvent) {
        event.source = true;
    }

    /// A gate weakly powers the block in front of its output with full power when on.
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
        let inputs = gate_inputs(&*level, pos, &g);
        if self.0.step(inputs, g.memory) != g.memory {
            level.schedule_block_tick(pos, event.state.block(), DELAY);
        }
    }

    fn tick_block(&self, level: &mut dyn LevelWriteHost, event: &mut TickBlockEvent) -> Flow {
        let pos = pos_of(event.pos);
        let Some(g) = state::gate(&*level, event.state) else {
            return Flow::Cancel;
        };
        let next = self.0.step(gate_inputs(&*level, pos, &g), g.memory);
        if next != g.memory {
            if let Some(ns) =
                state::with_gate(&*level, event.state, GateState { memory: next, ..g })
            {
                level.set_block(pos, ns, flags());
            }
            // A pulse, or an edge just seen, settles on the next step.
            if self.0.step(gate_inputs(&*level, pos, &g), next) != next {
                level.schedule_block_tick(pos, event.state.block(), DELAY);
            }
        }
        Flow::Cancel
    }

    /// A gate whose support went pops off.
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
                memory: gates::Memory::default(),
            },
        ))
    }
}

/// The items of the wire and the gates: placing one makes the block read its inputs at once,
/// so a NOT gate placed with nothing behind it turns on.
struct PlacingItem;

#[mod_api::horizon_part]
impl ItemPart for PlacingItem {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        let Some(level) = event.placer.and_then(|p| p.level(&*host)) else {
            return;
        };
        let _ = level.neighbor_changed(host, event.pos, event.state);
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let wire = r.block_key(WIRE);
        let _ = WIRE_KEY.set(wire);
        let _ = AIR_KEY.set(r.block_key("minecraft:air"));
        let _ = DUST_KEY.set(r.block_key("minecraft:redstone_wire"));
        r.block_part(wire, WirePart);
        let item = r.item_key(WIRE);
        r.item_part(item, PlacingItem);
        for (gate, id) in GATES {
            let key = r.block_key(id);
            r.block_part(key, GatePart(gate));
            let item = r.item_key(id);
            r.item_part(item, PlacingItem);
        }
    }
}
