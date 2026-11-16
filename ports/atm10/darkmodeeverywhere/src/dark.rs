//! Dark Mode Everywhere's model, apart from the game: the darkness presets the corner button
//! cycles, the screens it leaves alone, where a container's labels sit, and the saved form.

use std::collections::BTreeSet;

/// The darkness presets, in the order the button cycles them: a name's translation key and the
/// alpha of the black laid over a screen's panel (0 for off).
pub const PRESETS: &[(&str, u8)] = &[
    ("darkmodeeverywhere.preset.off", 0x00),
    ("darkmodeeverywhere.preset.dim", 0x78),
    ("darkmodeeverywhere.preset.dark", 0xA8),
    ("darkmodeeverywhere.preset.darkest", 0xD0),
];

/// The player's choices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dark {
    /// Index into `PRESETS`.
    pub preset: usize,
    /// Screens left alone: a menu type (`minecraft:furnace`) or a screen id (`options`).
    pub blacklist: BTreeSet<String>,
}

impl Default for Dark {
    fn default() -> Self {
        Dark {
            preset: 2,
            blacklist: BTreeSet::new(),
        }
    }
}

/// The name a screen goes by in the blacklist: its menu type when it has one, so two containers
/// on the inventory screen are told apart, else its screen id.
pub fn screen_key<'a>(id: &'a str, menu: Option<&'a str>) -> &'a str {
    menu.unwrap_or(id)
}

impl Dark {
    /// The colour laid over a screen's panel, ARGB, or None when the screen is drawn as it is.
    pub fn overlay(&self, key: &str) -> Option<u32> {
        let alpha = PRESETS.get(self.preset).map_or(0, |p| p.1);
        (alpha > 0 && !self.blacklist.contains(key)).then_some(u32::from(alpha) << 24)
    }

    /// The next preset, wrapping round to off.
    pub fn cycle(&mut self) {
        self.preset = (self.preset + 1) % PRESETS.len();
    }

    /// Puts a screen on the blacklist or takes it off.
    pub fn toggle_blacklist(&mut self, key: &str) {
        if !self.blacklist.remove(key) {
            self.blacklist.insert(key.to_owned());
        }
    }

    /// Reads the saved form: `preset <n>` and `blacklist <screen>` lines.
    pub fn parse(text: &str) -> Self {
        let mut dark = Dark::default();
        for line in text.lines() {
            match line.trim().split_once(' ') {
                Some(("preset", n)) => {
                    if let Ok(n) = n.trim().parse::<usize>()
                        && n < PRESETS.len()
                    {
                        dark.preset = n;
                    }
                }
                Some(("blacklist", key)) => {
                    dark.blacklist.insert(key.trim().to_owned());
                }
                _ => {}
            }
        }
        dark
    }

    pub fn write(&self) -> String {
        let mut out = format!("preset {}\n", self.preset);
        for key in &self.blacklist {
            out.push_str(&format!("blacklist {key}\n"));
        }
        out
    }
}

/// Where vanilla draws a container's title and the "Inventory" label, relative to the panel's
/// top left, for the menus whose labels are known (`AbstractContainerScreen`'s defaults: the title
/// at (8, 6), the inventory label at (8, panel height - 94)). None for any other menu.
pub fn label_spots(menu: &str, panel_height: f32) -> Option<([f32; 2], [f32; 2])> {
    let generic = menu
        .strip_prefix("minecraft:generic_9x")
        .and_then(|rows| rows.parse::<u8>().ok())
        .is_some_and(|rows| (1..=6).contains(&rows));
    (generic || menu == "minecraft:shulker_box" || menu == "minecraft:hopper")
        .then_some(([8.0, 6.0], [8.0, panel_height - 94.0]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_overlay_follows_the_preset_and_skips_blacklisted_screens() {
        let mut dark = Dark::default();
        assert_eq!(dark.overlay("minecraft:generic_9x3"), Some(0xA800_0000));
        dark.toggle_blacklist("minecraft:generic_9x3");
        assert_eq!(dark.overlay("minecraft:generic_9x3"), None);
        assert_eq!(dark.overlay("inventory"), Some(0xA800_0000));
        dark.cycle();
        assert_eq!(dark.overlay("inventory"), Some(0xD000_0000));
        dark.cycle();
        assert_eq!(
            dark.overlay("inventory"),
            None,
            "the cycle wraps round to off"
        );
    }

    #[test]
    fn a_container_goes_by_its_menu_and_a_plain_screen_by_its_id() {
        assert_eq!(
            screen_key("inventory", Some("minecraft:furnace")),
            "minecraft:furnace"
        );
        assert_eq!(screen_key("options", None), "options");
    }

    #[test]
    fn the_saved_form_reads_back() {
        let mut dark = Dark {
            preset: 1,
            ..Dark::default()
        };
        dark.toggle_blacklist("minecraft:crafter_3x3");
        dark.toggle_blacklist("options");
        assert_eq!(Dark::parse(&dark.write()), dark);
        assert_eq!(Dark::parse("preset 99\n").preset, Dark::default().preset);
    }

    #[test]
    fn a_chest_s_labels_sit_where_vanilla_draws_them() {
        // A single chest's panel is 166 high: "Inventory" at y 72; a large chest's 222: y 128.
        assert_eq!(
            label_spots("minecraft:generic_9x3", 166.0),
            Some(([8.0, 6.0], [8.0, 72.0]))
        );
        assert_eq!(
            label_spots("minecraft:generic_9x6", 222.0),
            Some(([8.0, 6.0], [8.0, 128.0]))
        );
        assert_eq!(label_spots("minecraft:furnace", 166.0), None);
        assert_eq!(label_spots("minecraft:generic_9x7", 240.0), None);
    }
}
