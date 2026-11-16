//! Mystical Agriculture for Horizon: essence crops that need farmland of their tier.
//!
//! Written from Mystical Agriculture's observed behaviour and public description, never from its
//! code (docs/modpacks.md, "Licences"). README.md lists what is ported and what is not.
//!
//! Each crop (`<name>_crop`) is a wheat-template block; its seeds (`<name>_seeds`) plant it and
//! it drops its essence (`<name>_essence`, 1 to 2) when fully grown, with the loot table in
//! `data/mysticalagriculture/loot_table/blocks/`. The part below stops a crop's random tick
//! (its growth) unless the farmland under it is of the crop's tier or higher.
use mod_api::{
    BlockPart, Flow, Hooks, LevelWriteHost, RandomTickEvent, Registrar, RegistrationContext,
};

pub mod rules;
use rules::*;

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
        for (name, _) in CROPS {
            let key = r.block_key(&format!("mysticalagriculture:{name}_crop"));
            r.block_part(key, Crop);
        }
    }
}
