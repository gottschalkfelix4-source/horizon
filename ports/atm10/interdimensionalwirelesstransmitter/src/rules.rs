//! The frequencies' shared state, apart from the game.
use std::collections::HashMap;

/// Frequencies, 0 to 15.
pub const FREQUENCIES: i32 = 16;
/// Ticks a transmitter's last word stays good: a missed beat or two (an unloaded chunk) is
/// bridged, a removed transmitter is forgotten.
pub const FRESH: i64 = 3;

/// Who is transmitting: dimension and position.
pub type Source = (String, [i32; 3]);

/// What each frequency carries, from every transmitter that spoke lately.
#[derive(Default, Debug)]
pub struct Air {
    heard: HashMap<i32, HashMap<Source, (i32, i64)>>,
}

impl Air {
    /// A transmitter says its strength (0 to 15) on `frequency` at game time `now`.
    pub fn say(&mut self, frequency: i32, source: Source, strength: i32, now: i64) {
        self.heard
            .entry(frequency.rem_euclid(FREQUENCIES))
            .or_default()
            .insert(source, (strength.clamp(0, 15), now));
    }

    /// What a receiver on `frequency` hears: the strongest fresh word.
    pub fn hear(&self, frequency: i32, now: i64) -> i32 {
        self.heard
            .get(&frequency.rem_euclid(FREQUENCIES))
            .into_iter()
            .flat_map(|m| m.values())
            .filter(|(_, at)| now - at <= FRESH)
            .map(|(s, _)| *s)
            .max()
            .unwrap_or(0)
    }

    /// Drops what nobody repeated since `now - FRESH`.
    pub fn forget(&mut self, now: i64) {
        for m in self.heard.values_mut() {
            m.retain(|_, (_, at)| now - *at <= FRESH);
        }
        self.heard.retain(|_, m| !m.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(dim: &str, x: i32) -> Source {
        (dim.to_owned(), [x, 64, 0])
    }

    #[test]
    fn a_receiver_in_another_dimension_hears_the_strongest_transmitter_on_its_frequency() {
        let mut air = Air::default();
        air.say(3, src("minecraft:overworld", 0), 7, 100);
        air.say(3, src("minecraft:the_nether", 5), 12, 100);
        air.say(4, src("minecraft:overworld", 9), 15, 100);
        assert_eq!(air.hear(3, 101), 12);
        assert_eq!(air.hear(4, 101), 15);
        assert_eq!(air.hear(5, 101), 0);
    }

    #[test]
    fn a_transmitter_that_stops_is_forgotten_after_a_few_ticks() {
        let mut air = Air::default();
        air.say(0, src("a", 0), 9, 100);
        assert_eq!(air.hear(0, 103), 9);
        assert_eq!(air.hear(0, 104), 0);
        air.forget(200);
        assert!(air.heard.is_empty());
    }

    #[test]
    fn a_transmitter_changing_its_mind_replaces_its_own_word_only() {
        let mut air = Air::default();
        air.say(0, src("a", 0), 9, 10);
        air.say(0, src("a", 1), 4, 10);
        air.say(0, src("a", 0), 2, 11);
        assert_eq!(air.hear(0, 11), 4);
    }

    #[test]
    fn frequencies_wrap() {
        let mut air = Air::default();
        air.say(16 + 2, src("a", 0), 6, 0);
        assert_eq!(air.hear(2, 0), 6);
        assert_eq!(air.hear(-14, 0), 6);
    }
}
