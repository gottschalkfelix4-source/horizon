//! Extreme Sound Muffler's model, apart from the game: which sounds are turned down, everywhere
//! or inside anchors, how loud a sound then plays, the list of sounds just heard, and the saved
//! form of it all.

use std::collections::{BTreeMap, VecDeque};

/// How many anchors there are.
pub const ANCHORS: usize = 10;
/// A new anchor's radius, in blocks.
pub const DEFAULT_RADIUS: u32 = 32;
/// The largest anchor radius, in blocks.
pub const MAX_RADIUS: u32 = 128;
/// How many recently heard sounds the list keeps.
pub const RECENT: usize = 64;

/// The volume factor of each muffled sound event (`minecraft:entity.cow.ambient` → 0.0 to 1.0).
pub type Volumes = BTreeMap<String, f32>;

/// An area where its own sounds are muffled.
#[derive(Clone, Debug, PartialEq)]
pub struct Anchor {
    /// The dimension it is in (`minecraft:overworld`).
    pub dimension: String,
    /// Its centre, in blocks.
    pub pos: [f64; 3],
    /// Its radius, in blocks (a sphere).
    pub radius: u32,
    pub sounds: Volumes,
}

impl Anchor {
    pub fn covers(&self, dimension: &str, at: [f64; 3]) -> bool {
        let d2: f64 = (0..3).map(|i| (at[i] - self.pos[i]).powi(2)).sum();
        self.dimension == dimension && d2 <= f64::from(self.radius).powi(2)
    }
}

/// Everything the player set.
#[derive(Clone, Debug, PartialEq)]
pub struct Muffler {
    /// Muffling on at all (the screen's on/off button).
    pub enabled: bool,
    pub global: Volumes,
    pub anchors: Vec<Option<Anchor>>,
}

impl Default for Muffler {
    fn default() -> Self {
        Muffler {
            enabled: true,
            global: Volumes::new(),
            anchors: vec![None; ANCHORS],
        }
    }
}

/// Where a sound plays: its dimension and position, or at the listener (`relative`).
#[derive(Clone, Copy, Debug)]
pub struct Place<'a> {
    pub dimension: &'a str,
    pub pos: [f64; 3],
    pub relative: bool,
}

impl Muffler {
    /// What `sound`'s volume is multiplied by at `place`: the global factor, times the lowest
    /// factor of the anchors around it. A relative sound (music, the UI) is in no anchor.
    pub fn factor(&self, sound: &str, place: Place<'_>) -> f32 {
        if !self.enabled {
            return 1.0;
        }
        let global = self.global.get(sound).copied().unwrap_or(1.0);
        let local = if place.relative {
            1.0
        } else {
            self.anchors
                .iter()
                .flatten()
                .filter(|a| a.covers(place.dimension, place.pos))
                .filter_map(|a| a.sounds.get(sound).copied())
                .fold(1.0_f32, f32::min)
        };
        (global * local).clamp(0.0, 1.0)
    }

    /// The volumes of `target`: `None` the global ones, `Some(n)` anchor `n`'s (None if unset).
    pub fn volumes_mut(&mut self, target: Option<usize>) -> Option<&mut Volumes> {
        match target {
            None => Some(&mut self.global),
            Some(n) => self.anchors.get_mut(n)?.as_mut().map(|a| &mut a.sounds),
        }
    }

    /// Mutes `sound` at `volume` in `target`, or lets it play again if it was muffled there.
    pub fn toggle(&mut self, target: Option<usize>, sound: &str, volume: f32) {
        if let Some(v) = self.volumes_mut(target)
            && v.remove(sound).is_none()
        {
            v.insert(sound.to_owned(), volume.clamp(0.0, 1.0));
        }
    }

    /// Moves a muffled sound's volume by `by` (a muffle's volume stays below full: 0.0 to 0.9).
    pub fn nudge(&mut self, target: Option<usize>, sound: &str, by: f32) {
        if let Some(v) = self.volumes_mut(target).and_then(|v| v.get_mut(sound)) {
            *v = ((*v + by) * 10.0).round().clamp(0.0, 9.0) / 10.0;
        }
    }

    /// Reads the saved form; lines it doesn't understand are skipped.
    pub fn parse(text: &str) -> Self {
        let mut m = Muffler::default();
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts.as_slice() {
                ["enabled", v] => m.enabled = *v != "false",
                ["global", id, v] => {
                    if let Ok(v) = v.parse::<f32>() {
                        m.global.insert((*id).to_owned(), v.clamp(0.0, 1.0));
                    }
                }
                ["anchor", n, dim, x, y, z, r] => {
                    let (Ok(n), Ok(x), Ok(y), Ok(z), Ok(r)) = (
                        n.parse::<usize>(),
                        x.parse::<f64>(),
                        y.parse::<f64>(),
                        z.parse::<f64>(),
                        r.parse::<u32>(),
                    ) else {
                        continue;
                    };
                    if let Some(slot) = m.anchors.get_mut(n) {
                        *slot = Some(Anchor {
                            dimension: (*dim).to_owned(),
                            pos: [x, y, z],
                            radius: r.clamp(1, MAX_RADIUS),
                            sounds: Volumes::new(),
                        });
                    }
                }
                ["anchor_sound", n, id, v] => {
                    if let (Ok(n), Ok(v)) = (n.parse::<usize>(), v.parse::<f32>())
                        && let Some(Some(a)) = m.anchors.get_mut(n)
                    {
                        a.sounds.insert((*id).to_owned(), v.clamp(0.0, 1.0));
                    }
                }
                _ => {}
            }
        }
        m
    }

    /// The saved form.
    pub fn write(&self) -> String {
        let mut out = format!("enabled {}\n", self.enabled);
        for (id, v) in &self.global {
            out.push_str(&format!("global {id} {v}\n"));
        }
        for (n, a) in self.anchors.iter().enumerate() {
            let Some(a) = a else { continue };
            out.push_str(&format!(
                "anchor {n} {} {} {} {} {}\n",
                a.dimension, a.pos[0], a.pos[1], a.pos[2], a.radius
            ));
            for (id, v) in &a.sounds {
                out.push_str(&format!("anchor_sound {n} {id} {v}\n"));
            }
        }
        out
    }
}

/// The sounds heard lately, the latest first, each once.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recent(pub VecDeque<String>);

impl Recent {
    pub fn heard(&mut self, sound: &str) {
        if self.0.front().is_some_and(|s| s == sound) {
            return;
        }
        self.0.retain(|s| s != sound);
        self.0.push_front(sound.to_owned());
        self.0.truncate(RECENT);
    }
}

/// The sounds of `list` whose id holds `search` (case-insensitive).
pub fn search<'a>(list: impl IntoIterator<Item = &'a String>, search: &str) -> Vec<&'a String> {
    let needle = search.trim().to_lowercase();
    list.into_iter()
        .filter(|s| needle.is_empty() || s.to_lowercase().contains(&needle))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OVERWORLD: &str = "minecraft:overworld";

    fn at(pos: [f64; 3]) -> Place<'static> {
        Place {
            dimension: OVERWORLD,
            pos,
            relative: false,
        }
    }

    fn anchor(pos: [f64; 3], radius: u32, sound: &str, v: f32) -> Anchor {
        Anchor {
            dimension: OVERWORLD.into(),
            pos,
            radius,
            sounds: [(sound.to_owned(), v)].into(),
        }
    }

    #[test]
    fn a_globally_muffled_sound_plays_at_its_volume_everywhere() {
        let mut m = Muffler::default();
        m.toggle(None, "minecraft:entity.cow.ambient", 0.3);
        assert_eq!(
            m.factor("minecraft:entity.cow.ambient", at([1e6, 0.0, 0.0])),
            0.3
        );
        assert_eq!(
            m.factor("minecraft:entity.pig.ambient", at([0.0, 0.0, 0.0])),
            1.0
        );
        m.toggle(None, "minecraft:entity.cow.ambient", 0.3);
        assert_eq!(
            m.factor("minecraft:entity.cow.ambient", at([0.0, 0.0, 0.0])),
            1.0
        );
    }

    #[test]
    fn an_anchor_muffles_only_inside_its_sphere_and_dimension() {
        let mut m = Muffler::default();
        m.anchors[0] = Some(anchor(
            [0.0, 64.0, 0.0],
            16,
            "minecraft:block.piston.extend",
            0.0,
        ));
        let piston = "minecraft:block.piston.extend";
        assert_eq!(
            m.factor(piston, at([10.0, 64.0, 12.0])),
            0.0,
            "15.6 blocks away"
        );
        assert_eq!(
            m.factor(piston, at([12.0, 64.0, 12.0])),
            1.0,
            "17 blocks away"
        );
        let nether = Place {
            dimension: "minecraft:the_nether",
            pos: [0.0, 64.0, 0.0],
            relative: false,
        };
        assert_eq!(m.factor(piston, nether), 1.0);
        let relative = Place {
            relative: true,
            ..at([0.0, 64.0, 0.0])
        };
        assert_eq!(m.factor(piston, relative), 1.0);
    }

    #[test]
    fn global_and_anchor_factors_multiply_and_overlapping_anchors_take_the_quietest() {
        let mut m = Muffler::default();
        let s = "minecraft:block.anvil.use";
        m.global.insert(s.into(), 0.5);
        m.anchors[1] = Some(anchor([0.0, 0.0, 0.0], 32, s, 0.6));
        m.anchors[2] = Some(anchor([5.0, 0.0, 0.0], 32, s, 0.2));
        assert!((m.factor(s, at([2.0, 0.0, 0.0])) - 0.1).abs() < 1e-6);
        m.enabled = false;
        assert_eq!(m.factor(s, at([2.0, 0.0, 0.0])), 1.0);
    }

    #[test]
    fn nudging_steps_by_tenths_and_stays_below_full() {
        let mut m = Muffler::default();
        let s = "minecraft:weather.rain";
        m.toggle(None, s, 0.0);
        m.nudge(None, s, 0.1);
        m.nudge(None, s, 0.1);
        assert_eq!(m.global[s], 0.2);
        for _ in 0..20 {
            m.nudge(None, s, 0.1);
        }
        assert_eq!(m.global[s], 0.9);
        for _ in 0..20 {
            m.nudge(None, s, -0.1);
        }
        assert_eq!(m.global[s], 0.0);
    }

    #[test]
    fn the_saved_form_reads_back() {
        let mut m = Muffler {
            enabled: false,
            ..Muffler::default()
        };
        m.global
            .insert("minecraft:entity.villager.ambient".into(), 0.0);
        m.anchors[3] = Some(anchor(
            [10.5, -3.0, 7.25],
            20,
            "minecraft:block.chest.open",
            0.4,
        ));
        m.anchors[9] = Some(anchor([0.0, 0.0, 0.0], 128, "minecraft:ambient.cave", 0.0));
        assert_eq!(Muffler::parse(&m.write()), m);
    }

    #[test]
    fn the_recent_list_keeps_each_sound_once_latest_first() {
        let mut r = Recent::default();
        for s in ["a:x", "a:y", "a:x", "a:x", "a:z"] {
            r.heard(s);
        }
        assert_eq!(r.0, ["a:z", "a:x", "a:y"]);
        for i in 0..100 {
            r.heard(&format!("a:{i}"));
        }
        assert_eq!(r.0.len(), RECENT);
        assert_eq!(r.0[0], "a:99");
    }

    #[test]
    fn search_matches_part_of_the_id_ignoring_case() {
        let list: Vec<String> = [
            "minecraft:entity.cow.ambient",
            "minecraft:block.stone.break",
        ]
        .map(String::from)
        .into();
        assert_eq!(search(&list, " COW"), [&list[0]]);
        assert_eq!(search(&list, "").len(), 2);
    }
}
