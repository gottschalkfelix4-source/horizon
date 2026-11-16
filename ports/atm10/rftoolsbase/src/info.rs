//! The Information Screen (`rftoolsbase:information_screen`): a small panel that reads the
//! energy of the block it is attached to and shows it as a bar, a number or a percentage, with
//! the rate it changes at.
use mod_api::{Compound, Tag};

/// How the screen shows what it reads; using the screen cycles through them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// Stored and capacity, `1.2M / 4M RF`.
    #[default]
    Text,
    /// A fill bar with the percentage.
    Percent,
    /// The change per tick, `+120 RF/t`.
    Rate,
}

impl Mode {
    pub fn next(self) -> Self {
        match self {
            Mode::Text => Mode::Percent,
            Mode::Percent => Mode::Rate,
            Mode::Rate => Mode::Text,
        }
    }

    pub fn code(self) -> i32 {
        self as i32
    }

    pub fn from_code(c: i32) -> Self {
        match c {
            1 => Mode::Percent,
            2 => Mode::Rate,
            _ => Mode::Text,
        }
    }
}

/// Ticks between two reads; the rate is the change over that window divided by it.
pub const SAMPLE_TICKS: i32 = 20;

/// RF in RFTools' short form: below a thousand as is, then K, M, G with one decimal where it
/// is not whole.
pub fn format_rf(value: i64) -> String {
    let abs = value.unsigned_abs();
    let (div, suffix) = match abs {
        0..=999 => return value.to_string(),
        1_000..=999_999 => (1_000u64, "K"),
        1_000_000..=999_999_999 => (1_000_000, "M"),
        _ => (1_000_000_000, "G"),
    };
    let tenths = abs * 10 / div;
    let sign = if value < 0 { "-" } else { "" };
    if tenths % 10 == 0 {
        format!("{sign}{}{suffix}", tenths / 10)
    } else {
        format!("{sign}{}.{}{suffix}", tenths / 10, tenths % 10)
    }
}

/// What the screen last read, saved in its compound and sent to clients that draw it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reading {
    pub energy: i64,
    pub capacity: i64,
    /// RF per tick over the last window; negative while draining.
    pub rate: i64,
    pub mode: Mode,
    /// Whether anything with energy is attached.
    pub present: bool,
}

impl Reading {
    /// A new read, `SAMPLE_TICKS` after `self`.
    pub fn next(self, now: Option<(i64, i64)>) -> Self {
        match now {
            None => Reading { mode: self.mode, ..Default::default() },
            Some((energy, capacity)) => Reading {
                energy,
                capacity,
                rate: if self.present { (energy - self.energy) / i64::from(SAMPLE_TICKS) } else { 0 },
                mode: self.mode,
                present: true,
            },
        }
    }

    /// The line the screen shows.
    pub fn line(&self) -> String {
        if !self.present {
            return "-".into();
        }
        match self.mode {
            Mode::Text => format!("{} / {} RF", format_rf(self.energy), format_rf(self.capacity)),
            Mode::Percent => {
                let pct = if self.capacity > 0 { self.energy * 100 / self.capacity } else { 0 };
                format!("{pct}%")
            }
            Mode::Rate => {
                let sign = if self.rate > 0 { "+" } else { "" };
                format!("{sign}{} RF/t", format_rf(self.rate))
            }
        }
    }

    pub fn save(&self, nbt: &mut Compound) {
        nbt.insert("info_energy".into(), Tag::Long(self.energy));
        nbt.insert("info_capacity".into(), Tag::Long(self.capacity));
        nbt.insert("info_rate".into(), Tag::Long(self.rate));
        nbt.insert("mode".into(), Tag::Int(self.mode.code()));
        nbt.insert("present".into(), Tag::Byte(i8::from(self.present)));
        nbt.insert("line".into(), Tag::String(self.line().as_str().into()));
    }

    pub fn load(nbt: &Compound) -> Self {
        let long = |k: &str| match nbt.get(&k.into()) {
            Some(Tag::Long(v)) => *v,
            Some(Tag::Int(v)) => i64::from(*v),
            _ => 0,
        };
        Reading {
            energy: long("info_energy"),
            capacity: long("info_capacity"),
            rate: long("info_rate"),
            mode: Mode::from_code(long("mode") as i32),
            present: matches!(nbt.get(&"present".into()), Some(Tag::Byte(1))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rf_is_shortened_with_one_decimal() {
        assert_eq!(format_rf(950), "950");
        assert_eq!(format_rf(1_000), "1K");
        assert_eq!(format_rf(1_250), "1.2K");
        assert_eq!(format_rf(4_000_000), "4M");
        assert_eq!(format_rf(-2_500_000), "-2.5M");
        assert_eq!(format_rf(3_000_000_000), "3G");
    }

    #[test]
    fn the_rate_is_the_change_over_the_window_and_starts_at_zero() {
        let first = Reading::default().next(Some((1_000, 100_000)));
        assert_eq!(first.rate, 0);
        let second = first.next(Some((3_400, 100_000)));
        assert_eq!(second.rate, 120);
        assert_eq!(Reading { mode: Mode::Rate, ..second }.line(), "+120 RF/t");
        let draining = second.next(Some((1_400, 100_000)));
        assert_eq!(Reading { mode: Mode::Rate, ..draining }.line(), "-100 RF/t");
    }

    #[test]
    fn modes_cycle_and_show_their_line() {
        let r = Reading { energy: 250_000, capacity: 1_000_000, present: true, ..Default::default() };
        assert_eq!(r.line(), "250K / 1M RF");
        assert_eq!(Reading { mode: r.mode.next(), ..r }.line(), "25%");
        assert_eq!(Mode::Rate.next(), Mode::Text);
        assert_eq!(Reading::default().line(), "-");
    }

    #[test]
    fn a_reading_round_trips_and_losing_the_source_keeps_the_mode() {
        let r = Reading { energy: 5, capacity: 10, rate: -1, mode: Mode::Percent, present: true };
        let mut nbt = Compound::new();
        r.save(&mut nbt);
        assert_eq!(Reading::load(&nbt), r);
        assert_eq!(r.next(None), Reading { mode: Mode::Percent, ..Default::default() });
    }
}
