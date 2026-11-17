//! `config/crashutilities.json`. The original keeps a NeoForge TOML config; the keys and defaults
//! here are assumed (README, "To check").
use serde::Deserialize;

/// The ground-item clear.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct ItemClear {
    /// Off by default, as in the original.
    pub enabled: bool,
    /// Items on the ground across all levels above which the clear runs.
    pub maximum: usize,
    /// How often the count is checked, in ticks.
    pub interval_ticks: u64,
}

impl Default for ItemClear {
    fn default() -> Self {
        Self {
            enabled: false,
            maximum: 1000,
            interval_ticks: 20 * 60,
        }
    }
}

/// The whole file.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub item_clear: ItemClear,
}

impl Config {
    /// The file's settings; a missing or broken file gives the defaults.
    pub fn parse(bytes: &[u8]) -> Self {
        serde_json::from_slice(bytes).unwrap_or_default()
    }
}
