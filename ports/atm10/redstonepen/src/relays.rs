//! What each relay outputs for its input. Pure logic, no host.

/// One relay's kind. A relay reads the signal behind it and drives the block in front.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relay {
    /// Full output while the back is powered.
    Relay,
    /// Full output while the back is not powered.
    Inverted,
    /// Toggles each time the back input turns on.
    Bistable,
    /// A short pulse each time the back input turns on.
    Pulse,
}

/// Every relay, with the block id it is registered under.
pub const RELAYS: [(Relay, &str); 4] = [
    (Relay::Relay, "redstonepen:relay"),
    (Relay::Inverted, "redstonepen:inverted_relay"),
    (Relay::Bistable, "redstonepen:bistable_relay"),
    (Relay::Pulse, "redstonepen:pulse_relay"),
];

/// Game ticks between the input changing and the output following.
pub const DELAY: i32 = 2;

/// What a relay keeps in its block state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Memory {
    /// The output is on.
    pub powered: bool,
    /// The stored bit of a bistable relay.
    pub memory: bool,
    /// The input as last seen, for the relays that act on its rising edge.
    pub input: bool,
}

impl Relay {
    /// The state the relay moves to with its back input `on`, from `now`.
    pub fn step(self, on: bool, now: Memory) -> Memory {
        let rising = on && !now.input;
        let edge = matches!(self, Relay::Bistable | Relay::Pulse);
        let mut next = Memory {
            powered: false,
            memory: now.memory,
            input: edge && on,
        };
        next.powered = match self {
            Relay::Relay => on,
            Relay::Inverted => !on,
            Relay::Bistable => {
                if rising {
                    next.memory = !now.memory;
                }
                next.memory
            }
            Relay::Pulse => rising,
        };
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_and_inverted_relays_follow_and_invert_their_input() {
        let off = Memory::default();
        assert!(Relay::Relay.step(true, off).powered);
        assert!(!Relay::Relay.step(false, off).powered);
        assert!(Relay::Inverted.step(false, off).powered);
        assert!(!Relay::Inverted.step(true, off).powered);
    }

    #[test]
    fn a_bistable_relay_toggles_once_per_rising_edge() {
        let r = Relay::Bistable;
        let on = r.step(true, Memory::default());
        assert!(on.powered);
        let held = r.step(true, on);
        assert!(held.powered);
        let released = r.step(false, held);
        assert!(released.powered);
        assert!(!r.step(true, released).powered);
    }

    #[test]
    fn a_pulse_relay_turns_off_on_the_next_step_while_the_input_stays_on() {
        let pulse = Relay::Pulse.step(true, Memory::default());
        assert!(pulse.powered);
        assert!(!Relay::Pulse.step(true, pulse).powered);
    }
}
