//! Steam: the boiler turns water and energy into steam, the turbine turns steam into energy.
//! Both are boxes of casing and valves (3 to 18 per side) found by [`crate::mb`]; the lowest
//! valve keeps the contents and every valve reaches them through the capabilities.
//!
//! The boiler (`boiler_casing`, `boiler_valve`, inside `superheating_element` or air): water in
//! through `FLUIDS`, energy in through `ENERGY` (the elements are electric here; Mekanism heats
//! it from a fission reactor or heaters, which are not ported), steam out through
//! `mekanism:chemicals`. Each element heats 500 mB of water per tick at 4 FE per mB.
//!
//! The turbine (`turbine_casing`, `turbine_valve`, inside `turbine_rotor`, `turbine_blade`,
//! `electromagnetic_coil` or air): steam in through `mekanism:chemicals`, energy out through
//! `ENERGY`. It burns up to 2 000 mB of steam per blade per tick for 4 FE per mB, scaled down
//! when there are fewer coils than half the blades. Stand-in numbers, see README.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart,
    BlockSide, Compound, ENERGY, EnergyHandler, FLUIDS, FluidResource, FluidStack, Host,
    LevelSide, Registrar, ResourceHandler, ServerHost, Transaction,
};

use crate::chemical::{CHEMICALS, ChemicalResource};
use crate::mb::{self, is_air, long, set_long};

pub const STEAM: &str = "mekanism:steam";
pub const WATER: &str = "minecraft:water";
/// FE per mB of steam, both ways (10 J at 2.5 J per FE).
pub const FE_PER_MB: i64 = 4;
pub const ELEMENT_MB: i64 = 500;
pub const BLADE_MB: i64 = 2_000;
/// mB held per interior block.
pub const PER_BLOCK: i64 = 16_000;
/// FE a turbine or boiler holds per interior block.
pub const ENERGY_PER_BLOCK: i64 = 100_000;

/// One tick of a boiler's data: water, energy and `elements` in, steam out.
pub fn boil(water: i64, steam: i64, energy: i64, elements: i64, steam_room: i64) -> (i64, i64, i64) {
    let n = water.min(steam_room).min(elements * ELEMENT_MB).min(energy / FE_PER_MB).max(0);
    (water - n, steam + n, energy - n * FE_PER_MB)
}

/// One tick of a turbine's data: steam and energy, with `blades` and `coils` inside and
/// `energy_room` left.
pub fn spin(steam: i64, energy: i64, blades: i64, coils: i64, energy_room: i64) -> (i64, i64) {
    if blades == 0 || coils == 0 {
        return (steam, energy);
    }
    // fewer coils than half the blades make less per mB
    let num = coils.min((blades + 1) / 2);
    let den = (blades + 1) / 2;
    let n = steam.min(blades * BLADE_MB).max(0);
    let made = n * FE_PER_MB * num / den;
    let n = if made > energy_room { energy_room * den / (FE_PER_MB * num).max(1) } else { n };
    (steam - n, energy + n * FE_PER_MB * num / den)
}

/// A valve of a boiler or a turbine, for the capabilities.
struct Valve {
    at: BlockSide,
    turbine: bool,
}

fn clamp(v: i64) -> u32 {
    v.clamp(0, i64::from(u32::MAX)) as u32
}

impl Valve {
    fn patch(&self, host: &mut dyn ServerHost, entity: mod_api::BlockEntityRef, f: impl FnOnce(&mut Compound)) {
        let mut patch = Compound::new();
        f(&mut patch);
        let _ = entity.merge_data(host, patch);
    }
}

impl EnergyHandler for Valve {
    fn amount(&self, host: &mut dyn ServerHost) -> u32 {
        mb::master(host, self.at).map_or(0, |(_, d)| clamp(long(&d, "energy")))
    }
    fn capacity(&self, host: &mut dyn ServerHost) -> u32 {
        mb::master(host, self.at).map_or(0, |(_, d)| clamp(long(&d, "energy_capacity")))
    }
    fn insert(&self, host: &mut dyn ServerHost, _tx: &Transaction, amount: u32) -> u32 {
        // the boiler's elements take energy; a turbine only gives it
        if self.turbine {
            return 0;
        }
        let Some((entity, d)) = mb::master(host, self.at) else { return 0 };
        let room = (long(&d, "energy_capacity") - long(&d, "energy")).max(0);
        let moved = i64::from(amount).min(room);
        if moved > 0 {
            self.patch(host, entity, |p| set_long(p, "energy", long(&d, "energy") + moved));
        }
        moved as u32
    }
    fn extract(&self, host: &mut dyn ServerHost, _tx: &Transaction, amount: u32) -> u32 {
        if !self.turbine {
            return 0;
        }
        let Some((entity, d)) = mb::master(host, self.at) else { return 0 };
        let moved = i64::from(amount).min(long(&d, "energy")).max(0);
        if moved > 0 {
            self.patch(host, entity, |p| set_long(p, "energy", long(&d, "energy") - moved));
        }
        moved as u32
    }
}

impl ResourceHandler<ChemicalResource> for Valve {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        1
    }
    fn resource(&self, host: &mut dyn ServerHost, _: usize) -> Option<ChemicalResource> {
        let (_, d) = mb::master(host, self.at)?;
        (long(&d, "steam") > 0).then(|| ChemicalResource(STEAM.into()))
    }
    fn amount(&self, host: &mut dyn ServerHost, _: usize) -> u32 {
        mb::master(host, self.at).map_or(0, |(_, d)| clamp(long(&d, "steam")))
    }
    fn capacity(&self, host: &mut dyn ServerHost, _: usize, r: &ChemicalResource) -> u32 {
        if r.0 != STEAM {
            return 0;
        }
        mb::master(host, self.at).map_or(0, |(_, d)| clamp(long(&d, "capacity")))
    }
    fn insert(&self, host: &mut dyn ServerHost, _tx: &Transaction, _: usize, r: &ChemicalResource, amount: u32) -> u32 {
        // a turbine takes steam in; a boiler only gives it
        if !self.turbine || r.0 != STEAM {
            return 0;
        }
        let Some((entity, d)) = mb::master(host, self.at) else { return 0 };
        let room = (long(&d, "capacity") - long(&d, "steam")).max(0);
        let moved = i64::from(amount).min(room);
        if moved > 0 {
            self.patch(host, entity, |p| set_long(p, "steam", long(&d, "steam") + moved));
        }
        moved as u32
    }
    fn extract(&self, host: &mut dyn ServerHost, _tx: &Transaction, _: usize, r: &ChemicalResource, amount: u32) -> u32 {
        if self.turbine || r.0 != STEAM {
            return 0;
        }
        let Some((entity, d)) = mb::master(host, self.at) else { return 0 };
        let moved = i64::from(amount).min(long(&d, "steam")).max(0);
        if moved > 0 {
            self.patch(host, entity, |p| set_long(p, "steam", long(&d, "steam") - moved));
        }
        moved as u32
    }
}

impl ResourceHandler<FluidResource> for Valve {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        1
    }
    fn resource(&self, host: &mut dyn ServerHost, _: usize) -> Option<FluidResource> {
        let (_, d) = mb::master(host, self.at)?;
        if long(&d, "water") == 0 {
            return None;
        }
        Some(FluidResource::of(&FluidStack::new(host.fluid_key(WATER)?, 1)))
    }
    fn amount(&self, host: &mut dyn ServerHost, _: usize) -> u32 {
        mb::master(host, self.at).map_or(0, |(_, d)| clamp(long(&d, "water")))
    }
    fn capacity(&self, host: &mut dyn ServerHost, _: usize, _: &FluidResource) -> u32 {
        if self.turbine {
            return 0;
        }
        mb::master(host, self.at).map_or(0, |(_, d)| clamp(long(&d, "capacity")))
    }
    fn insert(&self, host: &mut dyn ServerHost, _tx: &Transaction, _: usize, r: &FluidResource, amount: u32) -> u32 {
        let Some(key) = r.to_stack(1).key() else { return 0 };
        if self.turbine || key.id(&*host) != WATER {
            return 0;
        }
        let Some((entity, d)) = mb::master(host, self.at) else { return 0 };
        let room = (long(&d, "capacity") - long(&d, "water")).max(0);
        let moved = i64::from(amount).min(room);
        if moved > 0 {
            self.patch(host, entity, |p| set_long(p, "water", long(&d, "water") + moved));
        }
        moved as u32
    }
    fn extract(&self, _: &mut dyn ServerHost, _tx: &Transaction, _: usize, _: &FluidResource, _: u32) -> u32 {
        0
    }
}

pub const BOILER_VALVE: &str = "mekanism:boiler_valve";
pub const BOILER_CASING: &str = "mekanism:boiler_casing";
pub const ELEMENT: &str = "mekanism:superheating_element";
pub const TURBINE_VALVE: &str = "mekanism:turbine_valve";
pub const TURBINE_CASING: &str = "mekanism:turbine_casing";
pub const ROTOR: &str = "mekanism:turbine_rotor";
pub const BLADE: &str = "mekanism:turbine_blade";
pub const COIL: &str = "mekanism:electromagnetic_coil";

/// The valve block entity of the boiler (`turbine` false) or the turbine.
struct Part {
    turbine: bool,
}

#[mod_api::horizon_part]
impl BlockPart for Part {}

#[mod_api::horizon_part]
impl BlockEntityPart for Part {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        let valve = if self.turbine { TURBINE_VALVE } else { BOILER_VALVE };
        (event.block == valve).then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        let valve = if self.turbine { TURBINE_VALVE } else { BOILER_VALVE };
        if event.block == valve {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        if self.turbine {
            mb::tick_valve(
                host,
                event,
                TURBINE_VALVE,
                |id| matches!(id, Some(TURBINE_CASING | TURBINE_VALVE)),
                |id| is_air(id) || matches!(id, Some(ROTOR | BLADE | COIL)),
                |host, level, found, nbt| {
                    let count = |what: &str| {
                        found.interior.iter().filter(|p| mb::id_at(host, level, **p).as_deref() == Some(what)).count() as i64
                    };
                    let volume = found.interior.len() as i64;
                    set_long(nbt, "capacity", volume * PER_BLOCK);
                    set_long(nbt, "energy_capacity", volume * ENERGY_PER_BLOCK);
                    set_long(nbt, "blades", count(BLADE));
                    set_long(nbt, "coils", count(COIL));
                },
                |nbt| {
                    let (steam, energy) = spin(
                        long(nbt, "steam"),
                        long(nbt, "energy"),
                        long(nbt, "blades"),
                        long(nbt, "coils"),
                        long(nbt, "energy_capacity") - long(nbt, "energy"),
                    );
                    set_long(nbt, "steam", steam);
                    set_long(nbt, "energy", energy);
                },
            );
        } else {
            mb::tick_valve(
                host,
                event,
                BOILER_VALVE,
                |id| matches!(id, Some(BOILER_CASING | BOILER_VALVE)),
                |id| is_air(id) || id == Some(ELEMENT),
                |host, level, found, nbt| {
                    let elements = found
                        .interior
                        .iter()
                        .filter(|p| mb::id_at(host, level, **p).as_deref() == Some(ELEMENT))
                        .count() as i64;
                    let volume = found.interior.len() as i64;
                    set_long(nbt, "capacity", volume * PER_BLOCK);
                    set_long(nbt, "energy_capacity", volume * ENERGY_PER_BLOCK);
                    set_long(nbt, "elements", elements);
                },
                |nbt| {
                    let (water, steam, energy) = boil(
                        long(nbt, "water"),
                        long(nbt, "steam"),
                        long(nbt, "energy"),
                        long(nbt, "elements"),
                        long(nbt, "capacity") - long(nbt, "steam"),
                    );
                    set_long(nbt, "water", water);
                    set_long(nbt, "steam", steam);
                    set_long(nbt, "energy", energy);
                },
            );
        }
    }
}

/// Registers the two valves and their capabilities.
pub fn register(r: &mut dyn Registrar) {
    for (name, valve, turbine) in [("boiler_valve", BOILER_VALVE, false), ("turbine_valve", TURBINE_VALVE, true)] {
        let built = r.block_entity_type(name).blocks(&[valve]).build();
        let _ = r.block_entity_part(&built, Part { turbine });
        r.block_entity_capability(&ENERGY, &built, move |_, at| Some(Box::new(Valve { at, turbine })));
        r.block_entity_capability(&CHEMICALS, &built, move |_, at| Some(Box::new(Valve { at, turbine })));
        r.block_entity_capability(&FLUIDS, &built, move |_, at| Some(Box::new(Valve { at, turbine })));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_boiler_makes_steam_from_water_and_energy_up_to_its_elements() {
        // 2 elements: 1000 mB a tick at most
        assert_eq!(boil(5_000, 0, 100_000, 2, 100_000), (4_000, 1_000, 100_000 - 4_000));
        // limited by energy (4 FE a mB), by water, by room, and without elements
        assert_eq!(boil(5_000, 0, 40, 2, 100_000), (4_990, 10, 0));
        assert_eq!(boil(30, 0, 100_000, 2, 100_000).1, 30);
        assert_eq!(boil(5_000, 0, 100_000, 2, 7).1, 7);
        assert_eq!(boil(5_000, 0, 100_000, 0, 100_000).1, 0);
    }

    #[test]
    fn a_turbine_needs_blades_and_coils_and_makes_less_with_few_coils() {
        assert_eq!(spin(10_000, 0, 0, 4, 1_000_000), (10_000, 0));
        assert_eq!(spin(10_000, 0, 4, 0, 1_000_000), (10_000, 0));
        // 4 blades burn up to 8000 mB; 2 coils are enough for 4 blades
        assert_eq!(spin(10_000, 0, 4, 2, 1_000_000), (2_000, 32_000));
        // one coil for 4 blades: half the energy for the same steam
        assert_eq!(spin(10_000, 0, 4, 1, 1_000_000), (2_000, 16_000));
        // a full buffer stops it from burning what it cannot store
        assert_eq!(spin(10_000, 0, 4, 2, 800), (9_800, 800));
    }
}
