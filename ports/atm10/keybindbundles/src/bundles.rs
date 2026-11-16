//! KeyBind Bundles' model, apart from the game: the bundles (saved as text), which bindings a
//! bundle may hold, and which slice of the radial menu the pointer is on.

/// How many bundles there are; each has its own key in Controls (`bundle_1`...).
pub const BUNDLES: usize = 8;
/// The most bindings one bundle holds.
pub const MAX_ENTRIES: usize = 12;
/// Inside this radius, in GUI pixels, the pointer picks nothing: a release there fires nothing.
pub const DEAD_ZONE: f32 = 16.0;

/// The vanilla bindings a bundle can fire: Horizon's mod API clicks the movement, gameplay and
/// inventory ones (`ClientHost::click_mapping`), in Controls' order.
pub const CLICKABLE: &[&str] = &[
    "key.attack",
    "key.use",
    "key.pickItem",
    "key.jump",
    "key.sneak",
    "key.sprint",
    "key.drop",
    "key.inventory",
    "key.swapOffhand",
    "key.hotbar.1",
    "key.hotbar.2",
    "key.hotbar.3",
    "key.hotbar.4",
    "key.hotbar.5",
    "key.hotbar.6",
    "key.hotbar.7",
    "key.hotbar.8",
    "key.hotbar.9",
];

/// Every bundle's bindings, by bundle index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bundles(pub Vec<Vec<String>>);

impl Default for Bundles {
    fn default() -> Self {
        Bundles(vec![Vec::new(); BUNDLES])
    }
}

impl Bundles {
    /// Reads the saved form: `<bundle number> = <id>, <id>...`, one bundle a line, numbered from
    /// 1. Unknown bindings, numbers out of range and entries past `MAX_ENTRIES` are dropped.
    pub fn parse(text: &str) -> Self {
        let mut bundles = Bundles::default();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            let Some((n, ids)) = line.split_once('=') else {
                continue;
            };
            let Some(index) = n
                .trim()
                .parse::<usize>()
                .ok()
                .and_then(|n| n.checked_sub(1))
            else {
                continue;
            };
            let Some(bundle) = bundles.0.get_mut(index) else {
                continue;
            };
            for id in ids.split(',').map(str::trim) {
                if CLICKABLE.contains(&id)
                    && !bundle.iter().any(|e| e == id)
                    && bundle.len() < MAX_ENTRIES
                {
                    bundle.push(id.to_owned());
                }
            }
        }
        bundles
    }

    /// The saved form.
    pub fn write(&self) -> String {
        let mut out = String::from("# KeyBind Bundles: <bundle> = <binding>, <binding>...\n");
        for (i, bundle) in self.0.iter().enumerate() {
            if !bundle.is_empty() {
                out.push_str(&format!("{} = {}\n", i + 1, bundle.join(", ")));
            }
        }
        out
    }

    /// Adds `id` to bundle `index`, or takes it out. False when the bundle is full or the
    /// binding can't be fired.
    pub fn toggle(&mut self, index: usize, id: &str) -> bool {
        let Some(bundle) = self.0.get_mut(index) else {
            return false;
        };
        if let Some(at) = bundle.iter().position(|e| e == id) {
            bundle.remove(at);
            return true;
        }
        if bundle.len() >= MAX_ENTRIES || !CLICKABLE.contains(&id) {
            return false;
        }
        bundle.push(id.to_owned());
        true
    }
}

/// The slice of a radial menu of `count` slices around `center` under `at`: slice 0 starts at
/// the top and they go clockwise. None in the dead zone or with no slices.
pub fn slice_at(center: [f32; 2], at: [f32; 2], count: usize) -> Option<usize> {
    let (dx, dy) = (at[0] - center[0], at[1] - center[1]);
    if count == 0 || dx.hypot(dy) < DEAD_ZONE {
        return None;
    }
    let tau = std::f32::consts::TAU;
    // Angle from straight up, clockwise (screen y grows downwards).
    let angle = dx.atan2(-dy).rem_euclid(tau);
    Some(((angle / (tau / count as f32)) as usize).min(count - 1))
}

/// Slice `index` of `count`: its start and end angles, clockwise from the top, in radians.
pub fn slice_angles(index: usize, count: usize) -> (f32, f32) {
    let step = std::f32::consts::TAU / count.max(1) as f32;
    (index as f32 * step, (index + 1) as f32 * step)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pointer_picks_the_slice_it_is_in() {
        let c = [100.0, 100.0];
        // Four slices: top-right quarter first, then clockwise.
        assert_eq!(slice_at(c, [110.0, 40.0], 4), Some(0));
        assert_eq!(slice_at(c, [160.0, 110.0], 4), Some(1));
        assert_eq!(slice_at(c, [90.0, 160.0], 4), Some(2));
        assert_eq!(slice_at(c, [40.0, 90.0], 4), Some(3));
        // Three slices of 120 degrees: straight down is the middle of the second.
        assert_eq!(slice_at(c, [100.0, 150.0], 3), Some(1));
    }

    #[test]
    fn the_dead_zone_and_an_empty_bundle_pick_nothing() {
        let c = [100.0, 100.0];
        assert_eq!(slice_at(c, [105.0, 105.0], 4), None);
        assert_eq!(slice_at(c, [100.0, 0.0], 0), None);
    }

    #[test]
    fn bundles_read_back_from_their_saved_form() {
        let mut bundles = Bundles::default();
        assert!(bundles.toggle(0, "key.drop"));
        assert!(bundles.toggle(0, "key.swapOffhand"));
        assert!(bundles.toggle(2, "key.hotbar.9"));
        assert_eq!(Bundles::parse(&bundles.write()), bundles);
    }

    #[test]
    fn the_saved_form_drops_what_a_bundle_cannot_hold() {
        let text =
            "1 = key.drop, key.chat, key.drop, nonsense\n9 = key.jump\n0 = key.jump\nhello\n";
        let bundles = Bundles::parse(text);
        assert_eq!(bundles.0[0], ["key.drop"]);
        assert!(bundles.0[1..].iter().all(Vec::is_empty));
    }

    #[test]
    fn a_full_bundle_takes_no_more_and_a_second_toggle_removes() {
        let mut bundles = Bundles::default();
        for id in &CLICKABLE[..MAX_ENTRIES] {
            assert!(bundles.toggle(1, id));
        }
        assert!(!bundles.toggle(1, CLICKABLE[MAX_ENTRIES]));
        assert!(bundles.toggle(1, CLICKABLE[0]));
        assert_eq!(bundles.0[1].len(), MAX_ENTRIES - 1);
        assert!(
            !bundles.toggle(1, "key.chat"),
            "chat can't be clicked by a mod"
        );
    }
}
