//! Little Big Redstone for Horizon, first slice: the microchip. A microchip is a block holding a
//! small logic circuit; its four horizontal sides are the circuit's redstone inputs and outputs.
//!
//! Written from Little Big Redstone's observed behaviour and public description, never from its
//! code (docs/modpacks.md, "Licences"). What the port does, and what a harness against the
//! NeoForge original still has to confirm, is in README.md.
//!
//! How it works: the microchip item carries its circuit (a [`Program`] in its
//! `minecraft:custom_data`, under `program`). Placing it records the program, with its memory,
//! on the chunk the chip stands in (a chunk attachment, saved with the chunk) and runs it once.
//! A neighbour change schedules a tick one game tick later; the tick reads the four sides'
//! signals, steps the circuit and writes its outputs into the block state (`out_<side>`), which
//! `block_signal` reads back as a strong signal of 15 on that side.
use std::sync::OnceLock;

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockKey, BlockSignalEvent, BlockState,
    BlockUpdateFlags, Compound, Direction, Flow, Holder, Hooks, IsSignalSourceEvent, ItemPart,
    ItemStack, LevelHost, LevelTicksHost, LevelWriteHost, NeighborChangedEvent,
    OnBlockPlacedByEvent, Registrar, RegistrationContext, ServerHost, Tag, TickBlockEvent,
};

/// The microchip block and its item.
pub const MICROCHIP: &str = "little_big_redstone:microchip";

/// The chip's sides, in the order inputs, outputs and the state's properties use.
pub const SIDES: [Direction; 4] = [
    Direction::North,
    Direction::East,
    Direction::South,
    Direction::West,
];

/// The block-state property that says a side outputs.
const OUT: [&str; 4] = ["out_north", "out_east", "out_south", "out_west"];

/// One gate of a circuit. Gates read only gates before them, so one pass evaluates the
/// circuit; the latch and the toggle keep memory between passes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    /// The signal on a side (index into [`SIDES`]), on when above 0.
    Input(u8),
    Const(bool),
    Not(u16),
    And(u16, u16),
    Or(u16, u16),
    Xor(u16, u16),
    Nand(u16, u16),
    Nor(u16, u16),
    /// Set-reset latch: on once `set` is on, off once `reset` is on (reset wins).
    Latch {
        set: u16,
        reset: u16,
    },
    /// Flips on each rising edge of its input.
    Toggle(u16),
}

impl Gate {
    fn code(self) -> [i32; 3] {
        match self {
            Gate::Input(s) => [0, i32::from(s), 0],
            Gate::Const(v) => [1, i32::from(v), 0],
            Gate::Not(a) => [2, i32::from(a), 0],
            Gate::And(a, b) => [3, i32::from(a), i32::from(b)],
            Gate::Or(a, b) => [4, i32::from(a), i32::from(b)],
            Gate::Xor(a, b) => [5, i32::from(a), i32::from(b)],
            Gate::Nand(a, b) => [6, i32::from(a), i32::from(b)],
            Gate::Nor(a, b) => [7, i32::from(a), i32::from(b)],
            Gate::Latch { set, reset } => [8, i32::from(set), i32::from(reset)],
            Gate::Toggle(a) => [9, i32::from(a), 0],
        }
    }

    fn from_code(c: &[i32]) -> Option<Self> {
        let a = u16::try_from(c[1]).ok();
        let b = u16::try_from(c[2]).ok();
        Some(match c[0] {
            0 => Gate::Input(u8::try_from(c[1]).ok().filter(|s| *s < 4)?),
            1 => Gate::Const(c[1] != 0),
            2 => Gate::Not(a?),
            3 => Gate::And(a?, b?),
            4 => Gate::Or(a?, b?),
            5 => Gate::Xor(a?, b?),
            6 => Gate::Nand(a?, b?),
            7 => Gate::Nor(a?, b?),
            8 => Gate::Latch { set: a?, reset: b? },
            9 => Gate::Toggle(a?),
            _ => return None,
        })
    }

    /// The gates this one reads.
    fn reads(self) -> Vec<u16> {
        match self {
            Gate::Input(_) | Gate::Const(_) => vec![],
            Gate::Not(a) | Gate::Toggle(a) => vec![a],
            Gate::And(a, b)
            | Gate::Or(a, b)
            | Gate::Xor(a, b)
            | Gate::Nand(a, b)
            | Gate::Nor(a, b) => vec![a, b],
            Gate::Latch { set, reset } => vec![set, reset],
        }
    }
}

/// A chip's circuit: its gates and which gate drives each side (none: the side gives nothing).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Program {
    pub gates: Vec<Gate>,
    pub outputs: [Option<u16>; 4],
}

impl Program {
    /// A program is valid when every gate reads only gates before it and every output names a
    /// gate: one pass then evaluates it, with no combinational loop.
    pub fn is_valid(&self) -> bool {
        let n = self.gates.len();
        self.gates
            .iter()
            .enumerate()
            .all(|(i, g)| g.reads().iter().all(|&r| usize::from(r) < i))
            && self.outputs.iter().flatten().all(|&o| usize::from(o) < n)
    }

    /// Steps the circuit once with the sides' `inputs`, updating `memory` (one byte per gate:
    /// bit 0 the stored value, bit 1 the input seen last pass). The outputs, by side.
    pub fn step(&self, inputs: [bool; 4], memory: &mut Vec<u8>) -> [bool; 4] {
        memory.resize(self.gates.len(), 0);
        let mut values = Vec::with_capacity(self.gates.len());
        for (i, gate) in self.gates.iter().enumerate() {
            let at = |k: u16| values.get(usize::from(k)).copied().unwrap_or(false);
            let v = match *gate {
                Gate::Input(s) => inputs[usize::from(s)],
                Gate::Const(v) => v,
                Gate::Not(a) => !at(a),
                Gate::And(a, b) => at(a) && at(b),
                Gate::Or(a, b) => at(a) || at(b),
                Gate::Xor(a, b) => at(a) != at(b),
                Gate::Nand(a, b) => !(at(a) && at(b)),
                Gate::Nor(a, b) => !(at(a) || at(b)),
                Gate::Latch { set, reset } => {
                    let held = memory[i] & 1 != 0;
                    let v = !at(reset) && (at(set) || held);
                    memory[i] = u8::from(v);
                    v
                }
                Gate::Toggle(a) => {
                    let (held, last, now) = (memory[i] & 1 != 0, memory[i] & 2 != 0, at(a));
                    let v = held != (now && !last);
                    memory[i] = u8::from(v) | (u8::from(now) << 1);
                    v
                }
            };
            values.push(v);
        }
        self.outputs
            .map(|o| o.is_some_and(|k| values.get(usize::from(k)).copied().unwrap_or(false)))
    }

    fn to_nbt(&self) -> Compound {
        let gates = self.gates.iter().flat_map(|g| g.code()).collect();
        let outputs = self
            .outputs
            .iter()
            .map(|o| o.map_or(-1, i32::from))
            .collect();
        [
            ("gates".into(), Tag::IntArray(gates)),
            ("outputs".into(), Tag::IntArray(outputs)),
        ]
        .into()
    }

    /// The program saved as `{gates: [I; op, a, b, ...], outputs: [I; n, e, s, w]}` (-1 for no
    /// output). None for a malformed or invalid one.
    pub fn from_nbt(data: &Compound) -> Option<Self> {
        let (Some(Tag::IntArray(gates)), Some(Tag::IntArray(outputs))) =
            (data.get(&"gates".into()), data.get(&"outputs".into()))
        else {
            return None;
        };
        if gates.len() % 3 != 0 || outputs.len() != 4 {
            return None;
        }
        let gates = gates
            .chunks_exact(3)
            .map(Gate::from_code)
            .collect::<Option<Vec<_>>>()?;
        let mut out = [None; 4];
        for (slot, &o) in out.iter_mut().zip(outputs) {
            *slot = if o < 0 {
                None
            } else {
                Some(u16::try_from(o).ok()?)
            };
        }
        let program = Program {
            gates,
            outputs: out,
        };
        program.is_valid().then_some(program)
    }
}

/// Which sides a microchip's block state outputs on, read from its state string
/// (`little_big_redstone:microchip[out_east=true,...]`).
pub fn outputs_of(state: &str) -> [bool; 4] {
    let props = state
        .split_once('[')
        .map(|(_, p)| p.trim_end_matches(']'))
        .unwrap_or("");
    let mut out = [false; 4];
    for pair in props.split(',') {
        if let Some((k, v)) = pair.split_once('=')
            && let Some(i) = OUT.iter().position(|o| *o == k.trim())
        {
            out[i] = v.trim() == "true";
        }
    }
    out
}

/// One placed chip: where it is, its program and its gates' memory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chip {
    pub pos: [i32; 3],
    pub program: Program,
    pub memory: Vec<u8>,
}

/// The chips recorded on one chunk, saved as `{chips: [{pos, program, memory}]}`.
static CHIPS: AttachmentType<Vec<Chip>> = AttachmentType {
    namespace: "little_big_redstone",
    name: "chips",
    default: Vec::new,
    codec: Some(AttachmentCodec {
        encode: encode_chips,
        decode: decode_chips,
    }),
    copy_on_death: false,
    sync: false,
};

fn encode_chips(chips: &Vec<Chip>) -> Result<Compound, AttachmentError> {
    let list = chips
        .iter()
        .map(|c| {
            Tag::Compound(
                [
                    ("pos".into(), Tag::IntArray(c.pos.to_vec())),
                    ("program".into(), Tag::Compound(c.program.to_nbt())),
                    ("memory".into(), Tag::ByteArray(c.memory.clone())),
                ]
                .into(),
            )
        })
        .collect();
    Ok([("chips".into(), Tag::List(list))].into())
}

fn decode_chips(data: &Compound) -> Result<Vec<Chip>, AttachmentError> {
    let Some(Tag::List(list)) = data.get(&"chips".into()) else {
        return Err(AttachmentError::Decode);
    };
    list.iter()
        .map(|tag| {
            let Tag::Compound(c) = tag else {
                return Err(AttachmentError::Decode);
            };
            let (Some(Tag::IntArray(pos)), Some(Tag::Compound(program))) =
                (c.get(&"pos".into()), c.get(&"program".into()))
            else {
                return Err(AttachmentError::Decode);
            };
            if pos.len() != 3 {
                return Err(AttachmentError::Decode);
            }
            let program = Program::from_nbt(program).ok_or(AttachmentError::Decode)?;
            let memory = match c.get(&"memory".into()) {
                Some(Tag::ByteArray(m)) => m.clone(),
                _ => Vec::new(),
            };
            Ok(Chip {
                pos: [pos[0], pos[1], pos[2]],
                program,
                memory,
            })
        })
        .collect()
}

/// The program an item carries: `minecraft:custom_data` → `program`. A blank chip has none.
fn program_of(stack: &ItemStack) -> Program {
    let custom =
        stack
            .components()
            .and_then(|c| match c.as_nbt().get(&"minecraft:custom_data".into()) {
                Some(Tag::Compound(data)) => Some(data.clone()),
                _ => None,
            });
    custom
        .and_then(|data| match data.get(&"program".into()) {
            Some(Tag::Compound(p)) => Program::from_nbt(p),
            _ => None,
        })
        .unwrap_or_default()
}

static KEY: OnceLock<BlockKey> = OnceLock::new();

fn is_chip(state: BlockState) -> bool {
    KEY.get() == Some(&state.block())
}

fn chunk_of(pos: [i32; 3]) -> Holder {
    Holder::Chunk([pos[0] >> 4, pos[2] >> 4])
}

fn neighbour(pos: [i32; 3], side: Direction) -> [i32; 3] {
    match side {
        Direction::North => [pos[0], pos[1], pos[2] - 1],
        Direction::South => [pos[0], pos[1], pos[2] + 1],
        Direction::East => [pos[0] + 1, pos[1], pos[2]],
        Direction::West => [pos[0] - 1, pos[1], pos[2]],
        Direction::Up => [pos[0], pos[1] + 1, pos[2]],
        Direction::Down => [pos[0], pos[1] - 1, pos[2]],
    }
}

/// The microchip item: placing it records its program and runs it once.
struct ChipItem;

#[mod_api::horizon_part]
impl ItemPart for ChipItem {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        if !is_chip(event.state) {
            return;
        }
        let chip = Chip {
            pos: event.pos,
            program: program_of(event.stack),
            memory: Vec::new(),
        };
        let chunk = chunk_of(event.pos);
        let Ok(mut chips) = CHIPS.get(host.attachments(), chunk) else {
            return;
        };
        chips.retain(|c| c.pos != event.pos);
        chips.push(chip);
        let _ = CHIPS.set(host.attachments(), chunk, chips);
        // Run it now, so a chip with a constant output powers at once.
        if let Some(level) = event.placer.and_then(|p| p.level(host)) {
            level.block_tick(host, event.pos);
        }
    }
}

/// The microchip block: its sides read, step and give signals.
struct ChipBlock;

#[mod_api::horizon_part]
impl mod_api::BlockPart for ChipBlock {
    fn neighbor_changed(&self, level: &mut dyn LevelTicksHost, event: &mut NeighborChangedEvent) {
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        level.schedule_block_tick(pos, event.state.block(), 1);
    }

    fn tick_block(&self, level: &mut dyn LevelWriteHost, event: &mut TickBlockEvent) -> Flow {
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        let chunk = chunk_of(pos);
        let Ok(mut chips) = CHIPS.get(level.attachments(), chunk) else {
            return Flow::Continue;
        };
        let Some(chip) = chips.iter_mut().find(|c| c.pos == pos) else {
            return Flow::Continue;
        };
        let inputs = SIDES.map(|side| level.signal(neighbour(pos, side), side) > 0);
        let outputs = chip.program.step(inputs, &mut chip.memory);
        let _ = CHIPS.set(level.attachments(), chunk, chips);
        let mut state = event.state;
        for (name, on) in OUT.iter().zip(outputs) {
            if let Some(next) =
                level.state_with_property(state, name, if on { "true" } else { "false" })
            {
                state = next;
            }
        }
        if state != event.state {
            level.set_block(
                pos,
                state,
                BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
            );
        }
        Flow::Cancel
    }

    fn is_signal_source(&self, _: &dyn LevelHost, event: &mut IsSignalSourceEvent) {
        if is_chip(event.state) {
            event.source = true;
        }
    }

    fn block_signal(&self, level: &dyn LevelHost, event: &mut BlockSignalEvent) {
        if !is_chip(event.state) {
            return;
        }
        let Some(side) = SIDES.iter().position(|s| *s == event.face) else {
            event.signal = 0;
            return;
        };
        let on = level
            .state_string(event.state)
            .is_some_and(|s| outputs_of(&s)[side]);
        event.signal = if on { 15 } else { 0 };
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let block = r.block_key(MICROCHIP);
        let _ = KEY.set(block);
        r.block_part(block, ChipBlock);
        let item = r.item_key(MICROCHIP);
        r.item_part(item, ChipItem);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: usize = 0;
    const E: usize = 1;
    const S: usize = 2;

    fn sides(n: bool, e: bool, s: bool, w: bool) -> [bool; 4] {
        [n, e, s, w]
    }

    #[test]
    fn an_xor_chip_outputs_east_when_exactly_one_input_is_on() {
        let p = Program {
            gates: vec![Gate::Input(N as u8), Gate::Input(S as u8), Gate::Xor(0, 1)],
            outputs: [None, Some(2), None, None],
        };
        let mut mem = Vec::new();
        assert!(!p.step(sides(false, false, false, false), &mut mem)[E]);
        assert!(p.step(sides(true, false, false, false), &mut mem)[E]);
        assert!(p.step(sides(false, false, true, false), &mut mem)[E]);
        assert!(!p.step(sides(true, false, true, false), &mut mem)[E]);
    }

    #[test]
    fn a_latch_holds_after_set_goes_off_until_reset() {
        let p = Program {
            gates: vec![
                Gate::Input(N as u8),
                Gate::Input(S as u8),
                Gate::Latch { set: 0, reset: 1 },
            ],
            outputs: [None, Some(2), None, None],
        };
        let mut mem = Vec::new();
        assert!(p.step(sides(true, false, false, false), &mut mem)[E]);
        assert!(p.step(sides(false, false, false, false), &mut mem)[E]);
        assert!(!p.step(sides(false, false, true, false), &mut mem)[E]);
        assert!(!p.step(sides(false, false, false, false), &mut mem)[E]);
    }

    #[test]
    fn a_toggle_flips_once_per_rising_edge() {
        let p = Program {
            gates: vec![Gate::Input(N as u8), Gate::Toggle(0)],
            outputs: [None, Some(1), None, None],
        };
        let mut mem = Vec::new();
        let seen: Vec<bool> = [true, true, false, true, false, false]
            .into_iter()
            .map(|n| p.step(sides(n, false, false, false), &mut mem)[E])
            .collect();
        assert_eq!(seen, [true, true, true, false, false, false]);
    }

    #[test]
    fn a_gate_reading_a_later_gate_is_refused_when_loaded() {
        let looped = Program {
            gates: vec![Gate::Not(1), Gate::Not(0)],
            outputs: [Some(0), None, None, None],
        };
        assert_eq!(Program::from_nbt(&looped.to_nbt()), None);
        let fine = Program {
            gates: vec![Gate::Const(true), Gate::Not(0)],
            outputs: [Some(0), None, Some(1), None],
        };
        assert_eq!(Program::from_nbt(&fine.to_nbt()), Some(fine));
    }

    #[test]
    fn outputs_are_read_back_from_the_state_string() {
        let s = "little_big_redstone:microchip[out_east=true,out_north=false,out_south=true,out_west=false]";
        assert_eq!(outputs_of(s), [false, true, true, false]);
        assert_eq!(outputs_of("little_big_redstone:microchip"), [false; 4]);
    }
}
