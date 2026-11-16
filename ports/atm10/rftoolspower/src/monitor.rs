//! The Power Monitor: watches the energy of the block beside it and gives a redstone signal
//! when the fill crosses its threshold, and shows the fill in six steps.
use mod_api::{Compound, Tag};

/// Which side of the threshold sounds the alarm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Alarm {
    /// Signal while the fill is below the threshold (a backup generator's switch).
    #[default]
    Below,
    /// Signal while it is at or above it.
    Above,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Monitor {
    pub alarm: Alarm,
    /// Percent, 0 to 100.
    pub threshold: i32,
}

impl Default for Monitor {
    fn default() -> Self {
        Monitor { alarm: Alarm::Below, threshold: 50 }
    }
}

impl Monitor {
    /// The fill in percent, rounded down.
    pub fn percent(energy: i64, capacity: i64) -> i32 {
        if capacity <= 0 { 0 } else { (energy.clamp(0, capacity) * 100 / capacity) as i32 }
    }

    pub fn powered(&self, percent: i32) -> bool {
        match self.alarm {
            Alarm::Below => percent < self.threshold,
            Alarm::Above => percent >= self.threshold,
        }
    }

    /// The model's `level`, 0 (empty) to 5 (full).
    pub fn level(percent: i32) -> i32 {
        (percent.clamp(0, 100) * 5 + 50) / 100
    }

    pub fn load(nbt: &Compound) -> Self {
        let alarm = match nbt.get(&"alarmMode".into()) {
            Some(Tag::Int(1)) => Alarm::Above,
            _ => Alarm::Below,
        };
        let threshold = match nbt.get(&"alarmLevel".into()) {
            Some(Tag::Int(v)) => (*v).clamp(0, 100),
            _ => Monitor::default().threshold,
        };
        Monitor { alarm, threshold }
    }

    pub fn save(&self, nbt: &mut Compound) {
        nbt.insert("alarmMode".into(), Tag::Int(i32::from(self.alarm == Alarm::Above)));
        nbt.insert("alarmLevel".into(), Tag::Int(self.threshold));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn below_alarm_signals_under_the_threshold_and_above_at_or_over_it() {
        let low = Monitor { alarm: Alarm::Below, threshold: 30 };
        assert!(low.powered(Monitor::percent(290, 1000)));
        assert!(!low.powered(Monitor::percent(300, 1000)));
        let high = Monitor { alarm: Alarm::Above, threshold: 90 };
        assert!(high.powered(90) && !high.powered(89));
    }

    #[test]
    fn the_display_steps_through_six_levels() {
        assert_eq!([0, 9, 10, 50, 89, 90, 100].map(Monitor::level), [0, 0, 1, 3, 4, 5, 5]);
        assert_eq!(Monitor::percent(5, 0), 0);
    }

    #[test]
    fn settings_round_trip() {
        let m = Monitor { alarm: Alarm::Above, threshold: 75 };
        let mut nbt = Compound::new();
        m.save(&mut nbt);
        assert_eq!(Monitor::load(&nbt), m);
        assert_eq!(Monitor::load(&Compound::new()), Monitor::default());
    }
}
