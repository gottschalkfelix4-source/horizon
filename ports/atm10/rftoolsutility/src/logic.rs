//! The redstone logic blocks, as rules independent of the game: the Timer, the Counter and the
//! wireless channels the Redstone Transmitter and Receiver share.

/// Delays a Timer's click cycles through, in ticks.
pub const DELAYS: [i32; 9] = [1, 2, 5, 10, 20, 40, 100, 200, 600];
/// Targets a Counter's click cycles through.
pub const TARGETS: [i32; 10] = [1, 2, 3, 4, 5, 8, 10, 16, 32, 64];
/// Ticks a Counter's output stays on after it fires.
pub const COUNTER_PULSE: i32 = 2;
/// Ticks a transmitter's last report counts for: a transmitter that stops ticking (broken, or in
/// an unloaded chunk) stops sending after this.
pub const STALE_TICKS: i64 = 5;

/// The next of `steps` after `current` (the first when `current` is none of them).
pub fn next_step(steps: &[i32], current: i32) -> i32 {
    match steps.iter().position(|&s| s == current) {
        Some(i) => steps[(i + 1) % steps.len()],
        None => steps[0],
    }
}

/// A Timer: on for `pulse` ticks at the start of every `delay` ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timer {
    pub delay: i32,
    pub pulse: i32,
    pub phase: i32,
}

impl Default for Timer {
    fn default() -> Self {
        Self { delay: 20, pulse: 1, phase: 0 }
    }
}

impl Timer {
    /// One tick: whether the output is on during it.
    pub fn tick(&mut self) -> bool {
        let delay = self.delay.max(1);
        let on = self.phase < self.pulse.min(delay);
        self.phase = (self.phase + 1) % delay;
        on
    }
}

/// A Counter: counts rising edges of its input; on reaching `target` it resets and its output
/// is on for [`COUNTER_PULSE`] ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Counter {
    pub target: i32,
    pub count: i32,
    pub was_high: bool,
    pub out_ticks: i32,
}

impl Default for Counter {
    fn default() -> Self {
        Self { target: 4, count: 0, was_high: false, out_ticks: 0 }
    }
}

impl Counter {
    /// One tick with the input `high`: whether the output is on.
    pub fn tick(&mut self, high: bool) -> bool {
        if high && !self.was_high {
            self.count += 1;
            if self.count >= self.target {
                self.count = 0;
                self.out_ticks = COUNTER_PULSE;
            }
        }
        self.was_high = high;
        if self.out_ticks > 0 {
            self.out_ticks -= 1;
            true
        } else {
            false
        }
    }
}

/// What every Redstone Transmitter last reported, to the Receivers of the same channel.
#[derive(Debug, Default)]
pub struct Channels {
    /// (dimension, position, channel, level, game time of the report)
    reports: Vec<(String, [i32; 3], i32, i32, i64)>,
}

impl Channels {
    pub const fn new() -> Self {
        Self { reports: Vec::new() }
    }

    /// The transmitter at `pos` reports `level` on `channel` at `now`.
    pub fn report(&mut self, dim: &str, pos: [i32; 3], channel: i32, level: i32, now: i64) {
        self.reports.retain(|r| now - r.4 <= STALE_TICKS);
        match self.reports.iter_mut().find(|r| r.0 == dim && r.1 == pos) {
            Some(r) => *r = (dim.to_owned(), pos, channel, level, now),
            None => self.reports.push((dim.to_owned(), pos, channel, level, now)),
        }
    }

    /// The strongest fresh report on `channel`.
    pub fn level(&self, channel: i32, now: i64) -> i32 {
        self.reports
            .iter()
            .filter(|r| r.2 == channel && now - r.4 <= STALE_TICKS)
            .map(|r| r.3)
            .max()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timer_pulses_at_the_start_of_each_period() {
        let mut t = Timer { delay: 5, pulse: 2, phase: 0 };
        let out: Vec<bool> = (0..10).map(|_| t.tick()).collect();
        assert_eq!(out, [true, true, false, false, false, true, true, false, false, false]);
    }

    #[test]
    fn a_counter_fires_on_the_nth_rising_edge_not_while_the_input_stays_high() {
        let mut c = Counter { target: 3, ..Default::default() };
        let input = [true, true, false, true, false, true, false, false, false];
        let out: Vec<bool> = input.iter().map(|&i| c.tick(i)).collect();
        // Edges at ticks 0, 3 and 5; the third fires at tick 5 and holds for two ticks.
        assert_eq!(out, [false, false, false, false, false, true, true, false, false]);
        assert_eq!(c.count, 0);
    }

    #[test]
    fn a_receiver_hears_the_strongest_fresh_transmitter_of_its_channel() {
        let mut ch = Channels::new();
        ch.report("overworld", [0, 0, 0], 7, 4, 100);
        ch.report("the_nether", [5, 5, 5], 7, 9, 100);
        ch.report("overworld", [1, 0, 0], 8, 15, 100);
        assert_eq!(ch.level(7, 101), 9);
        assert_eq!(ch.level(8, 101), 15);
        // The nether transmitter keeps reporting 3; the other went silent.
        ch.report("the_nether", [5, 5, 5], 7, 3, 110);
        assert_eq!(ch.level(7, 111), 3);
        assert_eq!(ch.level(7, 120), 0);
    }

    #[test]
    fn a_click_steps_through_the_choices_and_wraps() {
        assert_eq!(next_step(&DELAYS, 20), 40);
        assert_eq!(next_step(&DELAYS, 600), 1);
        assert_eq!(next_step(&TARGETS, 7), 1);
    }
}
