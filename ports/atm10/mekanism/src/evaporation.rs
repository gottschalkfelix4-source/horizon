//! The thermal evaporation plant: a hollow box (3 to 18 per side) of evaporation blocks and valves,
//! empty inside. Water goes in through `FLUIDS` (tank 0), brine comes out (tank 1). With open sky
//! above the middle of the roof it turns 10 mB of water into brine per tick for each block of
//! inside height. Stand-in numbers (the original's needs 4x4 and heat sources and has an
//! efficiency by height and temperature), see README.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart,
    BlockSide, Compound, FLUIDS, FluidResource, FluidStack, Host, LevelSide, Registrar,
    ResourceHandler, ServerHost, Transaction,
};

use crate::mb::{self, is_air, long, set_long};

pub const VALVE: &str = "mekanism:thermal_evaporation_valve";
/// The output valve: only brine leaves through it. The plain valve only takes water in, so
/// brine cannot flow back out through the inlet into the tank that feeds it.
pub const OUTLET: &str = "mekanism:thermal_evaporation_outlet";
pub const CASING: &str = "mekanism:thermal_evaporation_block";
pub const WATER: &str = "minecraft:water";
pub const BRINE: &str = "mekanism:brine";
pub const PER_BLOCK: i64 = 16_000;
pub const MB_PER_HEIGHT: i64 = 10;

/// One tick: (water, brine) afterwards.
pub fn evaporate(water: i64, brine: i64, height: i64, sky: bool, brine_room: i64) -> (i64, i64) {
    if !sky {
        return (water, brine);
    }
    let n = water.min(height * MB_PER_HEIGHT).min(brine_room).max(0);
    (water - n, brine + n)
}

fn clamp(v: i64) -> u32 {
    v.clamp(0, i64::from(u32::MAX)) as u32
}

struct Valve {
    at: BlockSide,
    out: bool,
}

impl Valve {
    fn add(&self, host: &mut dyn ServerHost, field: &str, delta: i64) {
        let Some((entity, d)) = mb::master(host, self.at) else { return };
        let mut patch = Compound::new();
        set_long(&mut patch, field, long(&d, field) + delta);
        let _ = entity.merge_data(host, patch);
    }
}

impl ResourceHandler<FluidResource> for Valve {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        2
    }
    fn resource(&self, host: &mut dyn ServerHost, i: usize) -> Option<FluidResource> {
        let (_, d) = mb::master(host, self.at)?;
        let (field, id) = if i == 0 { ("water", WATER) } else { ("brine", BRINE) };
        if long(&d, field) == 0 {
            return None;
        }
        Some(FluidResource::of(&FluidStack::new(host.fluid_key(id)?, 1)))
    }
    fn amount(&self, host: &mut dyn ServerHost, i: usize) -> u32 {
        let field = if i == 0 { "water" } else { "brine" };
        mb::master(host, self.at).map_or(0, |(_, d)| clamp(long(&d, field)))
    }
    fn capacity(&self, host: &mut dyn ServerHost, i: usize, r: &FluidResource) -> u32 {
        let Some(key) = r.to_stack(1).key() else { return 0 };
        let id = key.id(&*host);
        if id != if i == 0 { WATER } else { BRINE } {
            return 0;
        }
        mb::master(host, self.at).map_or(0, |(_, d)| clamp(long(&d, "capacity")))
    }
    fn insert(&self, host: &mut dyn ServerHost, _tx: &Transaction, i: usize, r: &FluidResource, amount: u32) -> u32 {
        // water in only
        let Some(key) = r.to_stack(1).key() else { return 0 };
        if self.out || i != 0 || key.id(&*host) != WATER {
            return 0;
        }
        let Some((_, d)) = mb::master(host, self.at) else { return 0 };
        let moved = i64::from(amount).min((long(&d, "capacity") - long(&d, "water")).max(0));
        if moved > 0 {
            self.add(host, "water", moved);
        }
        moved as u32
    }
    fn extract(&self, host: &mut dyn ServerHost, _tx: &Transaction, i: usize, r: &FluidResource, amount: u32) -> u32 {
        // brine out only
        let Some(key) = r.to_stack(1).key() else { return 0 };
        if !self.out || i != 1 || key.id(&*host) != BRINE {
            return 0;
        }
        let Some((_, d)) = mb::master(host, self.at) else { return 0 };
        let moved = i64::from(amount).min(long(&d, "brine")).max(0);
        if moved > 0 {
            self.add(host, "brine", -moved);
        }
        moved as u32
    }
}

struct Plant;

#[mod_api::horizon_part]
impl BlockPart for Plant {}

#[mod_api::horizon_part]
impl BlockEntityPart for Plant {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == VALVE || event.block == OUTLET).then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == VALVE || event.block == OUTLET {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        mb::tick_valve(
            host,
            event,
            VALVE,
            |id| matches!(id, Some(CASING | VALVE | OUTLET)),
            is_air,
            |host, level, found, nbt| {
                set_long(nbt, "capacity", found.interior.len() as i64 * PER_BLOCK);
                set_long(nbt, "height", i64::from(found.max[1] - found.min[1] - 1));
                let roof = [
                    (found.min[0] + found.max[0]) / 2,
                    found.max[1] + 1,
                    (found.min[2] + found.max[2]) / 2,
                ];
                set_long(nbt, "sky", i64::from(level.can_see_sky(host, roof).unwrap_or(false)));
            },
            |nbt| {
                let (water, brine) = evaporate(
                    long(nbt, "water"),
                    long(nbt, "brine"),
                    long(nbt, "height"),
                    long(nbt, "sky") == 1,
                    long(nbt, "capacity") - long(nbt, "brine"),
                );
                set_long(nbt, "water", water);
                set_long(nbt, "brine", brine);
            },
        );
    }
}

pub fn register(r: &mut dyn Registrar) {
    let _ = r.declare_fluid("brine");
    let built = r.block_entity_type("thermal_evaporation_valve").blocks(&[VALVE]).build();
    let _ = r.block_entity_part(&built, Plant);
    r.block_entity_capability(&FLUIDS, &built, |_, at| Some(Box::new(Valve { at, out: false })));
    let outlet = r.block_entity_type("thermal_evaporation_outlet").blocks(&[OUTLET]).build();
    let _ = r.block_entity_part(&outlet, Plant);
    r.block_entity_capability(&FLUIDS, &outlet, |_, at| Some(Box::new(Valve { at, out: true })));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brine_needs_sky_and_scales_with_height() {
        assert_eq!(evaporate(1_000, 0, 4, true, 10_000), (960, 40));
        assert_eq!(evaporate(1_000, 0, 4, false, 10_000), (1_000, 0));
        assert_eq!(evaporate(15, 0, 4, true, 10_000), (0, 15));
        assert_eq!(evaporate(1_000, 0, 4, true, 7), (993, 7));
    }
}
