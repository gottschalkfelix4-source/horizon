//! The Endergenic Generator: a pearl juggler. A redstone pulse on an idle generator starts it
//! charging (it draws RF every tick); a charged generator that receives an ender pearl holds
//! it, and a pulse while holding fires the pearl to the generator it is linked to. Each
//! arrival at a charged generator pays RF, more the more often that pearl has been passed on,
//! so a well-timed ring of generators gives far more than it costs.
//! A pearl that arrives at a generator that isn't charging is lost; one held too long is lost.
//! Pearl Injectors put new pearls in.
use mod_api::{Compound, Tag};

/// RF a charging generator draws each tick.
pub const CHARGE_RF_PER_TICK: i32 = 1_000;
/// Ticks a held pearl lasts before it is lost.
pub const HOLD_TIMEOUT: i32 = 100;
/// RF the generator holds.
pub const CAPACITY: i32 = 5_000_000;
/// RF paid when a pearl arrives, by how many times it was passed on before (the last entry
/// for every pass after).
pub const RF_PER_HIT: [i32; 10] = [0, 100, 150, 200, 400, 800, 1_600, 3_200, 6_400, 12_800];
/// Ticks a fired pearl flies per block of distance.
pub const TICKS_PER_BLOCK: i32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Idle,
    Charging,
    /// Holding a pearl that has been passed on `age` times, for `ticks` so far.
    Holding { age: i32, ticks: i32 },
}

/// What happened, for the glue to carry out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// RF drawn from the buffer (charging).
    pub spent: i32,
    /// RF paid into the buffer (an arrival).
    pub gained: i32,
    /// A pearl leaves for the linked generator, with this age.
    pub fired: Option<i32>,
    /// A pearl was lost.
    pub lost: bool,
}

pub fn rf_for(age: i32) -> i32 {
    RF_PER_HIT[(age.max(0) as usize).min(RF_PER_HIT.len() - 1)]
}

/// A redstone pulse (rising edge).
pub fn pulse(state: &mut State) -> Outcome {
    match *state {
        State::Idle => {
            *state = State::Charging;
            Outcome::default()
        }
        State::Charging => Outcome::default(),
        State::Holding { age, .. } => {
            *state = State::Idle;
            Outcome { fired: Some(age + 1), ..Default::default() }
        }
    }
}

/// A pearl of `age` arrives (from another generator or an injector, age 0).
pub fn arrive(state: &mut State, age: i32) -> Outcome {
    match *state {
        State::Charging => {
            *state = State::Holding { age, ticks: 0 };
            Outcome { gained: rf_for(age), ..Default::default() }
        }
        _ => Outcome { lost: true, ..Default::default() },
    }
}

/// One tick with `energy` in the buffer.
pub fn tick(state: &mut State, energy: i32) -> Outcome {
    match *state {
        State::Idle => Outcome::default(),
        State::Charging => {
            if energy < CHARGE_RF_PER_TICK {
                // Out of power: the charge collapses.
                *state = State::Idle;
                Outcome::default()
            } else {
                Outcome { spent: CHARGE_RF_PER_TICK, ..Default::default() }
            }
        }
        State::Holding { age, ticks } => {
            if ticks + 1 >= HOLD_TIMEOUT {
                *state = State::Idle;
                Outcome { lost: true, ..Default::default() }
            } else {
                *state = State::Holding { age, ticks: ticks + 1 };
                Outcome::default()
            }
        }
    }
}

impl State {
    pub fn save(&self, nbt: &mut Compound) {
        let (code, age, ticks) = match *self {
            State::Idle => (0, 0, 0),
            State::Charging => (1, 0, 0),
            State::Holding { age, ticks } => (2, age, ticks),
        };
        nbt.insert("chargingMode".into(), Tag::Int(code));
        nbt.insert("pearlAge".into(), Tag::Int(age));
        nbt.insert("holdTicks".into(), Tag::Int(ticks));
    }

    pub fn load(nbt: &Compound) -> Self {
        let int = |k: &str| match nbt.get(&k.into()) {
            Some(Tag::Int(v)) => *v,
            _ => 0,
        };
        match int("chargingMode") {
            1 => State::Charging,
            2 => State::Holding { age: int("pearlAge"), ticks: int("holdTicks") },
            _ => State::Idle,
        }
    }
}

/// Pearls on their way to a generator: `(age, ticks left)`, saved as `incoming: [I; age, t, ...]`.
pub fn load_incoming(nbt: &Compound) -> Vec<(i32, i32)> {
    match nbt.get(&"incoming".into()) {
        Some(Tag::IntArray(v)) => v.chunks_exact(2).map(|c| (c[0], c[1])).collect(),
        _ => Vec::new(),
    }
}

pub fn save_incoming(nbt: &mut Compound, incoming: &[(i32, i32)]) {
    nbt.insert(
        "incoming".into(),
        Tag::IntArray(incoming.iter().flat_map(|&(a, t)| [a, t]).collect()),
    );
}

/// Flight time between two generators.
pub fn flight_ticks(from: [i32; 3], to: [i32; 3]) -> i32 {
    let d = (0..3).map(|i| f64::from(from[i] - to[i]).powi(2)).sum::<f64>().sqrt();
    (d.ceil() as i32 * TICKS_PER_BLOCK).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pulse_charges_a_pearl_is_held_and_the_next_pulse_fires_it_older() {
        let mut s = State::Idle;
        pulse(&mut s);
        assert_eq!(s, State::Charging);
        assert_eq!(tick(&mut s, 50_000).spent, CHARGE_RF_PER_TICK);
        let got = arrive(&mut s, 3);
        assert_eq!(got.gained, 200);
        assert_eq!(s, State::Holding { age: 3, ticks: 0 });
        assert_eq!(pulse(&mut s).fired, Some(4));
        assert_eq!(s, State::Idle);
    }

    #[test]
    fn a_pearl_is_lost_at_an_idle_generator_and_after_the_hold_timeout() {
        let mut s = State::Idle;
        assert!(arrive(&mut s, 0).lost);
        let mut s = State::Holding { age: 1, ticks: 0 };
        let lost = (0..HOLD_TIMEOUT).filter(|_| tick(&mut s, 0).lost).count();
        assert_eq!(lost, 1);
        assert_eq!(s, State::Idle);
    }

    #[test]
    fn charging_collapses_without_power_and_old_pearls_pay_the_top_rate() {
        let mut s = State::Charging;
        assert_eq!(tick(&mut s, 10), Outcome::default());
        assert_eq!(s, State::Idle);
        assert_eq!(rf_for(42), 12_800);
    }

    #[test]
    fn state_and_pearls_in_flight_round_trip() {
        let mut nbt = Compound::new();
        let s = State::Holding { age: 5, ticks: 17 };
        s.save(&mut nbt);
        save_incoming(&mut nbt, &[(2, 7), (0, 1)]);
        assert_eq!(State::load(&nbt), s);
        assert_eq!(load_incoming(&nbt), vec![(2, 7), (0, 1)]);
        assert_eq!(flight_ticks([0, 0, 0], [3, 0, 4]), 5);
    }
}
