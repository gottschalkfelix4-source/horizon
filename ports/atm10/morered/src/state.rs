//! Reading and writing this mod's block-state properties through a level host.
//!
//! The level host reads a state as its serialised form (`morered:diode[attach=down,...]`) and
//! changes one property at a time. A decoded state is cached by its value, since `block_signal`
//! asks for it very often.
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use mod_api::{BlockState, Direction, LevelHost};

use crate::gates::Memory;
use crate::orient::{self, ALL, bit};

/// A decoded wire state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wire {
    /// Which faces of the cell carry wire (`orient::bit`).
    pub faces: u8,
    /// The power it carries, 0 to 15.
    pub power: u8,
}

/// A decoded gate state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GateState {
    pub attach: Direction,
    pub rotation: u8,
    pub memory: Memory,
}

/// The properties of a serialised state, `ns:id[a=b,c=d]`, in order.
pub fn properties(serialised: &str) -> Vec<(&str, &str)> {
    let Some(start) = serialised.find('[') else {
        return Vec::new();
    };
    let inner = serialised[start + 1..].trim_end_matches(']');
    inner
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .collect()
}

fn flag(v: &str) -> bool {
    v == "true"
}

/// A wire from its properties: one boolean per direction name and `power`.
pub fn wire_from(props: &[(&str, &str)]) -> Wire {
    let mut w = Wire { faces: 0, power: 0 };
    for (k, v) in props {
        if *k == "power" {
            w.power = v.parse().unwrap_or(0);
        } else if let Some(d) = orient::parse(k)
            && flag(v)
        {
            w.faces |= bit(d);
        }
    }
    w
}

/// A gate from its properties: `attach`, `rotation`, `powered`, `memory`, `input`.
pub fn gate_from(props: &[(&str, &str)]) -> GateState {
    let mut g = GateState {
        attach: Direction::Down,
        rotation: 0,
        memory: Memory::default(),
    };
    for (k, v) in props {
        match *k {
            "attach" => g.attach = orient::parse(v).unwrap_or(Direction::Down),
            "rotation" => g.rotation = v.parse().unwrap_or(0),
            "powered" => g.memory.powered = flag(v),
            "memory" => g.memory.memory = flag(v),
            "input" => g.memory.input = flag(v),
            _ => {}
        }
    }
    g
}

#[derive(Clone, Copy)]
enum Decoded {
    Wire(Wire),
    Gate(GateState),
}

fn cache() -> &'static RwLock<HashMap<BlockState, Decoded>> {
    static CACHE: OnceLock<RwLock<HashMap<BlockState, Decoded>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn decode(level: &dyn LevelHost, state: BlockState, wire: bool) -> Option<Decoded> {
    if let Some(d) = cache().read().ok().and_then(|c| c.get(&state).copied()) {
        return Some(d);
    }
    let s = level.state_string(state)?;
    let props = properties(&s);
    let d = if wire {
        Decoded::Wire(wire_from(&props))
    } else {
        Decoded::Gate(gate_from(&props))
    };
    if let Ok(mut c) = cache().write() {
        c.insert(state, d);
    }
    Some(d)
}

/// The wire `state` is.
pub fn wire(level: &dyn LevelHost, state: BlockState) -> Option<Wire> {
    match decode(level, state, true)? {
        Decoded::Wire(w) => Some(w),
        Decoded::Gate(_) => None,
    }
}

/// The gate `state` is.
pub fn gate(level: &dyn LevelHost, state: BlockState) -> Option<GateState> {
    match decode(level, state, false)? {
        Decoded::Gate(g) => Some(g),
        Decoded::Wire(_) => None,
    }
}

fn bool_str(b: bool) -> &'static str {
    if b { "true" } else { "false" }
}

const POWERS: [&str; 16] = [
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15",
];
const ROTATIONS: [&str; 4] = ["0", "1", "2", "3"];

/// `base` (any state of the wire block) with `w`'s properties.
pub fn with_wire(level: &dyn LevelHost, base: BlockState, w: Wire) -> Option<BlockState> {
    let mut s = level.state_with_property(base, "power", POWERS[usize::from(w.power.min(15))])?;
    for d in ALL {
        s = level.state_with_property(s, orient::name(d), bool_str(w.faces & bit(d) != 0))?;
    }
    Some(s)
}

/// `base` (any state of the gate's block) with `g`'s properties.
pub fn with_gate(level: &dyn LevelHost, base: BlockState, g: GateState) -> Option<BlockState> {
    let s = level.state_with_property(base, "attach", orient::name(g.attach))?;
    let s = level.state_with_property(s, "rotation", ROTATIONS[usize::from(g.rotation & 3)])?;
    let s = level.state_with_property(s, "powered", bool_str(g.memory.powered))?;
    let s = level.state_with_property(s, "memory", bool_str(g.memory.memory))?;
    level.state_with_property(s, "input", bool_str(g.memory.input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_serialised_wire_state_decodes_its_faces_and_power() {
        let s = "morered:red_alloy_wire[down=true,east=false,north=true,power=7,south=false,up=false,west=false]";
        let w = wire_from(&properties(s));
        assert_eq!(w.power, 7);
        assert_eq!(w.faces, bit(Direction::Down) | bit(Direction::North));
    }

    #[test]
    fn a_serialised_gate_state_decodes_its_orientation_and_memory() {
        let s = "morered:latch[attach=east,input=false,memory=true,powered=true,rotation=2]";
        let g = gate_from(&properties(s));
        assert_eq!(g.attach, Direction::East);
        assert_eq!(g.rotation, 2);
        assert!(g.memory.memory && g.memory.powered && !g.memory.input);
    }
}
