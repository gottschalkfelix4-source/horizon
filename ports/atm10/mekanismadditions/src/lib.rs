//! Mekanism Additions for Horizon: plastic blocks, glow panels and balloons are data
//! (`data/mekanismadditions/horizon`, written by `gen_data.py`); this crate has no behaviour of
//! its own yet. Obsidian TNT, the baby mobs, the walkie-talkie and the balloons' entity are not ported.
use mod_api::Hooks;

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {}
