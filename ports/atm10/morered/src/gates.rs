//! What each logic gate outputs for its inputs. Pure logic, no host.

/// One gate's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    /// Output when the back is off.
    Not,
    /// Output when the back is on: a one-way buffer.
    Diode,
    /// Output when left, back and right are all on.
    And,
    /// Output when left and right are both on (the back is not an input).
    And2,
    /// Output unless left, back and right are all on.
    Nand,
    /// Output unless left and right are both on.
    Nand2,
    /// Output when any of left, back, right is on.
    Or,
    /// Output when none of left, back, right is on.
    Nor,
    /// Output when exactly one of left and right is on.
    Xor,
    /// Output when left and right agree.
    Xnor,
    /// The back selects: off passes the left input, on passes the right.
    Multiplexer,
    /// Set by the left input, reset by the right; holds in between. Both on resets.
    Latch,
    /// Toggles each time the back input turns on.
    TFlipFlop,
    /// A short pulse each time the back input turns on.
    Pulse,
}

/// Every gate, with the block id it is registered under.
pub const GATES: [(Gate, &str); 14] = [
    (Gate::Not, "morered:not_gate"),
    (Gate::Diode, "morered:diode"),
    (Gate::And, "morered:and_gate"),
    (Gate::And2, "morered:and_2_gate"),
    (Gate::Nand, "morered:nand_gate"),
    (Gate::Nand2, "morered:nand_2_gate"),
    (Gate::Or, "morered:or_gate"),
    (Gate::Nor, "morered:nor_gate"),
    (Gate::Xor, "morered:xor_gate"),
    (Gate::Xnor, "morered:xnor_gate"),
    (Gate::Multiplexer, "morered:multiplexer"),
    (Gate::Latch, "morered:latch"),
    (Gate::TFlipFlop, "morered:t_flip_flop"),
    (Gate::Pulse, "morered:pulse_gate"),
];

/// Game ticks between an input changing and the output following (one redstone tick).
pub const DELAY: i32 = 2;

/// Whether each input side is powered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Inputs {
    pub left: bool,
    pub back: bool,
    pub right: bool,
}

/// What a gate keeps in its block state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Memory {
    /// The output is on.
    pub powered: bool,
    /// The stored bit of a latch or flip-flop.
    pub memory: bool,
    /// The back input as last seen, for the gates that act on its rising edge.
    pub input: bool,
}

impl Gate {
    /// Whether this gate acts on the back input turning on rather than its level.
    fn edge(self) -> bool {
        matches!(self, Gate::TFlipFlop | Gate::Pulse)
    }

    /// The state the gate moves to with `inputs`, from `now`.
    pub fn step(self, inputs: Inputs, now: Memory) -> Memory {
        let Inputs { left, back, right } = inputs;
        let rising = back && !now.input;
        let mut next = Memory {
            powered: false,
            memory: now.memory,
            input: if self.edge() { back } else { false },
        };
        next.powered = match self {
            Gate::Not => !back,
            Gate::Diode => back,
            Gate::And => left && back && right,
            Gate::And2 => left && right,
            Gate::Nand => !(left && back && right),
            Gate::Nand2 => !(left && right),
            Gate::Or => left || back || right,
            Gate::Nor => !(left || back || right),
            Gate::Xor => left != right,
            Gate::Xnor => left == right,
            Gate::Multiplexer => {
                if back {
                    right
                } else {
                    left
                }
            }
            Gate::Latch => {
                if right {
                    next.memory = false;
                } else if left {
                    next.memory = true;
                }
                next.memory
            }
            Gate::TFlipFlop => {
                if rising {
                    next.memory = !now.memory;
                }
                next.memory
            }
            Gate::Pulse => rising,
        };
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn io(left: bool, back: bool, right: bool) -> Inputs {
        Inputs { left, back, right }
    }

    #[test]
    fn the_combinational_gates_follow_their_truth_tables() {
        let off = Memory::default();
        let out = |g: Gate, i| g.step(i, off).powered;
        assert!(out(Gate::Not, io(false, false, false)));
        assert!(!out(Gate::Not, io(true, true, true)));
        assert!(out(Gate::And, io(true, true, true)));
        assert!(!out(Gate::And, io(true, false, true)));
        assert!(out(Gate::And2, io(true, false, true)));
        assert!(out(Gate::Nand, io(true, false, true)));
        assert!(!out(Gate::Nand2, io(true, false, true)));
        assert!(out(Gate::Or, io(false, true, false)));
        assert!(!out(Gate::Nor, io(false, true, false)));
        assert!(out(Gate::Xor, io(true, true, false)));
        assert!(!out(Gate::Xor, io(true, false, true)));
        assert!(out(Gate::Xnor, io(false, true, false)));
        assert!(out(Gate::Multiplexer, io(true, false, false)));
        assert!(!out(Gate::Multiplexer, io(true, true, false)));
        assert!(out(Gate::Multiplexer, io(false, true, true)));
    }

    #[test]
    fn a_latch_holds_between_set_and_reset() {
        let set = Gate::Latch.step(io(true, false, false), Memory::default());
        assert!(set.powered);
        let held = Gate::Latch.step(io(false, false, false), set);
        assert!(held.powered);
        let reset = Gate::Latch.step(io(true, false, true), held);
        assert!(!reset.powered);
    }

    #[test]
    fn a_flip_flop_toggles_once_per_rising_edge() {
        let g = Gate::TFlipFlop;
        let on = g.step(io(false, true, false), Memory::default());
        assert!(on.powered);
        let still = g.step(io(false, true, false), on);
        assert!(still.powered, "a held input does not toggle again");
        let released = g.step(io(false, false, false), still);
        assert!(released.powered);
        let off = g.step(io(false, true, false), released);
        assert!(!off.powered);
    }

    #[test]
    fn a_pulse_gate_turns_off_on_the_next_step_while_the_input_stays_on() {
        let g = Gate::Pulse;
        let pulse = g.step(io(false, true, false), Memory::default());
        assert!(pulse.powered);
        let after = g.step(io(false, true, false), pulse);
        assert!(!after.powered);
    }
}
