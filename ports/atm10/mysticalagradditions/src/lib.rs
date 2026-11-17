//! Mystical Agradditions for Horizon: tier 6 farmland and the awakened crops that need it.
//!
//! Written from the mod's data files and its observed behaviour, never from its code
//! (docs/modpacks.md, "Licences"). Blocks and items are data (`data/mysticalagradditions/horizon`,
//! written by `gen_data.py`); the part below stops an awakened crop's random tick (its growth)
//! unless the farmland under it is of its tier or higher, as the Mystical Agriculture port does
//! for its own crops. README.md lists what is ported and what is not.
use mod_api::{
    BlockPart, Flow, Hooks, LevelWriteHost, RandomTickEvent, Registrar, RegistrationContext,
};

pub mod rules;
use rules::*;

#[cfg(test)]
mod data_check;

struct Crop;

#[mod_api::horizon_part]
impl BlockPart for Crop {
    fn random_tick(&self, level: &mut dyn LevelWriteHost, event: &mut RandomTickEvent) -> Flow {
        let below = [event.pos.x, event.pos.y - 1, event.pos.z];
        let farmland = level
            .block_state(below)
            .and_then(|s| level.state_block(s).map(str::to_owned));
        let crop = level.state_block(event.state).map(str::to_owned);
        match (farmland, crop) {
            (Some(f), Some(c)) if can_grow(&f, &c) => Flow::Continue,
            _ => Flow::Cancel,
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for name in CROPS {
            let key = r.block_key(&format!("mysticalagradditions:{name}_crop"));
            r.block_part(key, Crop);
        }
    }
}
