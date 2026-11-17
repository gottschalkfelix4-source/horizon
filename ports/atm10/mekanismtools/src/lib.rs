//! Mekanism Tools for Horizon: the tool and armor sets are data (`data/mekanismtools/horizon/item`,
//! written by `gen_data.py`); this crate has no behaviour of its own yet. Paxels, shields and the
//! armor's effects (refined glowstone's night vision and so on) are not ported.
use mod_api::Hooks;

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {}
