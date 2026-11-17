//! The fission reactor: a hollow box (3 to 18 per side) of reactor casing and ports around
//! fission fuel assemblies and control rod assemblies (air elsewhere inside). Fissile fuel goes
//! in through `mekanism:chemicals` (tank 0), water through `FLUIDS`; steam (tank 1) and nuclear
//! waste (tank 2) come out through `mekanism:chemicals`. The lowest port keeps the contents.
//!
//! Stand-in model (Mekanism's has heat, damage and meltdowns, which are not ported): every
//! assembly burns up to 1 mB of fuel a tick, but only as much as the water can cool: each mB of
//! fuel turns 1 000 mB of water into steam. Nothing runs without a control rod assembly. The
//! ids are `mekanism:` here, where the original's are `mekanismgenerators:` (a chemical
//! capability is typed by the Mekanism port's crate, so the reactor lives in it).
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart,
    BlockSide, Compound, FLUIDS, FluidResource, FluidStack, Host, LevelSide, Registrar,
    ResourceHandler, ServerHost, Transaction,
};

use crate::chemical::{CHEMICALS, ChemicalResource};
use crate::mb::{self, is_air, long, set_long};
use crate::steam::STEAM;

pub const FUEL: &str = "mekanism:fissile_fuel";
pub const WASTE: &str = "mekanism:nuclear_waste";
pub const WATER: &str = "minecraft:water";
pub const PORT: &str = "mekanism:fission_reactor_port";
pub const CASING: &str = "mekanism:fission_reactor_casing";
pub const ASSEMBLY: &str = "mekanism:fission_fuel_assembly";
pub const ROD: &str = "mekanism:control_rod_assembly";
/// mB of steam one mB of fuel makes.
pub const STEAM_PER_FUEL: i64 = 1_000;
/// mB of fuel or waste held per assembly, and of water or steam per interior block.
pub const PER_ASSEMBLY: i64 = 8_000;
pub const PER_BLOCK: i64 = 16_000;

/// The contents of a reactor, in mB.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Tanks {
    pub fuel: i64,
    pub waste: i64,
    pub water: i64,
    pub steam: i64,
}

/// One tick: what the reactor holds afterwards, given its assemblies and rods and the room for
/// steam and waste.
pub fn burn(t: Tanks, assemblies: i64, rods: i64, steam_room: i64, waste_room: i64) -> Tanks {
    if rods == 0 || assemblies == 0 {
        return t;
    }
    let want = t.fuel.min(assemblies).min(waste_room).max(0);
    let steam = (want * STEAM_PER_FUEL).min(t.water).min(steam_room).max(0);
    // only as much fuel burns as the steam it made accounts for
    let burned = (steam + STEAM_PER_FUEL - 1) / STEAM_PER_FUEL;
    Tanks { fuel: t.fuel - burned, waste: t.waste + burned, water: t.water - steam, steam: t.steam + steam }
}

fn clamp(v: i64) -> u32 {
    v.clamp(0, i64::from(u32::MAX)) as u32
}

/// A port of the reactor, for the capabilities.
struct Port {
    at: BlockSide,
}

/// The saved field of a chemical tank index: 0 fuel, 1 steam, 2 waste.
const FIELDS: [(&str, &str); 3] = [("fuel", FUEL), ("steam", STEAM), ("waste", WASTE)];

impl Port {
    fn add(&self, host: &mut dyn ServerHost, field: &str, delta: i64) {
        let Some((entity, d)) = mb::master(host, self.at) else { return };
        let mut patch = Compound::new();
        set_long(&mut patch, field, long(&d, field) + delta);
        let _ = entity.merge_data(host, patch);
    }

    fn cap_of(&self, d: &Compound, field: &str) -> i64 {
        match field {
            "fuel" | "waste" => long(d, "fuel_capacity"),
            _ => long(d, "capacity"),
        }
    }
}

impl ResourceHandler<ChemicalResource> for Port {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        FIELDS.len()
    }
    fn resource(&self, host: &mut dyn ServerHost, i: usize) -> Option<ChemicalResource> {
        let (field, id) = *FIELDS.get(i)?;
        let (_, d) = mb::master(host, self.at)?;
        (long(&d, field) > 0).then(|| ChemicalResource(id.into()))
    }
    fn amount(&self, host: &mut dyn ServerHost, i: usize) -> u32 {
        let Some((field, _)) = FIELDS.get(i) else { return 0 };
        mb::master(host, self.at).map_or(0, |(_, d)| clamp(long(&d, field)))
    }
    fn capacity(&self, host: &mut dyn ServerHost, i: usize, r: &ChemicalResource) -> u32 {
        let Some((field, id)) = FIELDS.get(i) else { return 0 };
        if r.0 != *id {
            return 0;
        }
        mb::master(host, self.at).map_or(0, |(_, d)| clamp(self.cap_of(&d, field)))
    }
    fn insert(&self, host: &mut dyn ServerHost, _tx: &Transaction, i: usize, r: &ChemicalResource, amount: u32) -> u32 {
        // only the fuel tank takes anything in
        if i != 0 || r.0 != FUEL {
            return 0;
        }
        let Some((_, d)) = mb::master(host, self.at) else { return 0 };
        let moved = i64::from(amount).min((self.cap_of(&d, "fuel") - long(&d, "fuel")).max(0));
        if moved > 0 {
            self.add(host, "fuel", moved);
        }
        moved as u32
    }
    fn extract(&self, host: &mut dyn ServerHost, _tx: &Transaction, i: usize, r: &ChemicalResource, amount: u32) -> u32 {
        let Some((field, id)) = FIELDS.get(i) else { return 0 };
        if i == 0 || r.0 != *id {
            return 0;
        }
        let Some((_, d)) = mb::master(host, self.at) else { return 0 };
        let moved = i64::from(amount).min(long(&d, field)).max(0);
        if moved > 0 {
            self.add(host, field, -moved);
        }
        moved as u32
    }
}

impl ResourceHandler<FluidResource> for Port {
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
        mb::master(host, self.at).map_or(0, |(_, d)| clamp(long(&d, "capacity")))
    }
    fn insert(&self, host: &mut dyn ServerHost, _tx: &Transaction, _: usize, r: &FluidResource, amount: u32) -> u32 {
        let Some(key) = r.to_stack(1).key() else { return 0 };
        if key.id(&*host) != WATER {
            return 0;
        }
        let Some((_, d)) = mb::master(host, self.at) else { return 0 };
        let moved = i64::from(amount).min((long(&d, "capacity") - long(&d, "water")).max(0));
        if moved > 0 {
            self.add(host, "water", moved);
        }
        moved as u32
    }
    fn extract(&self, _: &mut dyn ServerHost, _tx: &Transaction, _: usize, _: &FluidResource, _: u32) -> u32 {
        0
    }
}

/// The port's block entity.
struct ReactorPort;

#[mod_api::horizon_part]
impl BlockPart for ReactorPort {}

#[mod_api::horizon_part]
impl BlockEntityPart for ReactorPort {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == PORT).then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == PORT {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        mb::tick_valve(
            host,
            event,
            PORT,
            |id| matches!(id, Some(CASING | PORT)),
            |id| is_air(id) || matches!(id, Some(ASSEMBLY | ROD)),
            |host, level, found, nbt| {
                let count = |what: &str| {
                    found.interior.iter().filter(|p| mb::id_at(host, level, **p).as_deref() == Some(what)).count() as i64
                };
                let assemblies = count(ASSEMBLY);
                set_long(nbt, "capacity", found.interior.len() as i64 * PER_BLOCK);
                set_long(nbt, "fuel_capacity", assemblies * PER_ASSEMBLY);
                set_long(nbt, "assemblies", assemblies);
                set_long(nbt, "rods", count(ROD));
            },
            |nbt| {
                let t = burn(
                    Tanks {
                        fuel: long(nbt, "fuel"),
                        waste: long(nbt, "waste"),
                        water: long(nbt, "water"),
                        steam: long(nbt, "steam"),
                    },
                    long(nbt, "assemblies"),
                    long(nbt, "rods"),
                    long(nbt, "capacity") - long(nbt, "steam"),
                    long(nbt, "fuel_capacity") - long(nbt, "waste") - long(nbt, "fuel"),
                );
                set_long(nbt, "fuel", t.fuel);
                set_long(nbt, "waste", t.waste);
                set_long(nbt, "water", t.water);
                set_long(nbt, "steam", t.steam);
            },
        );
    }
}

/// Registers the port and its capabilities.
pub fn register(r: &mut dyn Registrar) {
    let built = r.block_entity_type("fission_reactor_port").blocks(&[PORT]).build();
    let _ = r.block_entity_part(&built, ReactorPort);
    r.block_entity_capability(&CHEMICALS, &built, |_, at| Some(Box::new(Port { at })));
    r.block_entity_capability(&FLUIDS, &built, |_, at| Some(Box::new(Port { at })));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tanks(fuel: i64, water: i64) -> Tanks {
        Tanks { fuel, waste: 0, water, steam: 0 }
    }

    #[test]
    fn a_reactor_needs_a_rod_and_burns_only_what_its_water_can_cool() {
        assert_eq!(burn(tanks(10, 50_000), 4, 0, 1_000_000, 1_000), tanks(10, 50_000));
        // 4 assemblies burn 4 mB and make 4000 steam from 4000 water
        let t = burn(tanks(10, 50_000), 4, 1, 1_000_000, 1_000);
        assert_eq!(t, Tanks { fuel: 6, waste: 4, water: 46_000, steam: 4_000 });
        // 1500 mB of water cools 1.5 mB of fuel: 2 burn (rounded up), steam is what the water gave
        let t = burn(tanks(10, 1_500), 4, 1, 1_000_000, 1_000);
        assert_eq!((t.fuel, t.waste, t.water, t.steam), (8, 2, 0, 1_500));
        // no room for steam: nothing burns
        assert_eq!(burn(tanks(10, 50_000), 4, 1, 0, 1_000), tanks(10, 50_000));
        // waste full: nothing burns
        assert_eq!(burn(tanks(10, 50_000), 4, 1, 1_000_000, 0), tanks(10, 50_000));
    }
}
