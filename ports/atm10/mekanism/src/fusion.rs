//! The fusion reactor: a hollow box (3 to 18 per side) of fusion reactor frames and ports, empty
//! inside. Deuterium (tank 0) and tritium (tank 1) go in through `mekanism:chemicals`; energy
//! comes out through `ENERGY`. Stand-in model: no laser, plasma heat or injection rate; every
//! tick up to 10 mB of each fuse into 20 000 FE per mB pair. See README.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart,
    BlockSide, Compound, ENERGY, EnergyHandler, Host, LevelSide, Registrar, ResourceHandler,
    ServerHost, Transaction,
};

use crate::chemical::{CHEMICALS, ChemicalResource};
use crate::mb::{self, is_air, long, set_long};

pub const DEUTERIUM: &str = "mekanism:deuterium";
pub const TRITIUM: &str = "mekanism:tritium";
pub const PORT: &str = "mekanism:fusion_reactor_port";
pub const FRAME: &str = "mekanism:fusion_reactor_frame";
pub const MB_PER_TICK: i64 = 10;
pub const FE_PER_PAIR: i64 = 20_000;
pub const FUEL_PER_BLOCK: i64 = 1_000;
pub const ENERGY_PER_BLOCK: i64 = 1_000_000;

/// One tick: (deuterium, tritium, energy) afterwards.
pub fn fuse(d: i64, t: i64, energy: i64, energy_room: i64) -> (i64, i64, i64) {
    let n = d.min(t).min(MB_PER_TICK).min(energy_room / FE_PER_PAIR).max(0);
    (d - n, t - n, energy + n * FE_PER_PAIR)
}

fn clamp(v: i64) -> u32 {
    v.clamp(0, i64::from(u32::MAX)) as u32
}

const FIELDS: [(&str, &str); 2] = [("deuterium", DEUTERIUM), ("tritium", TRITIUM)];

struct Port {
    at: BlockSide,
}

impl Port {
    fn add(&self, host: &mut dyn ServerHost, field: &str, delta: i64) {
        let Some((entity, d)) = mb::master(host, self.at) else { return };
        let mut patch = Compound::new();
        set_long(&mut patch, field, long(&d, field) + delta);
        let _ = entity.merge_data(host, patch);
    }
}

impl EnergyHandler for Port {
    fn amount(&self, host: &mut dyn ServerHost) -> u32 {
        mb::master(host, self.at).map_or(0, |(_, d)| clamp(long(&d, "energy")))
    }
    fn capacity(&self, host: &mut dyn ServerHost) -> u32 {
        mb::master(host, self.at).map_or(0, |(_, d)| clamp(long(&d, "energy_capacity")))
    }
    fn insert(&self, _: &mut dyn ServerHost, _tx: &Transaction, _: u32) -> u32 {
        0
    }
    fn extract(&self, host: &mut dyn ServerHost, _tx: &Transaction, amount: u32) -> u32 {
        let Some((_, d)) = mb::master(host, self.at) else { return 0 };
        let moved = i64::from(amount).min(long(&d, "energy")).max(0);
        if moved > 0 {
            self.add(host, "energy", -moved);
        }
        moved as u32
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
        let Some((_, id)) = FIELDS.get(i) else { return 0 };
        if r.0 != *id {
            return 0;
        }
        mb::master(host, self.at).map_or(0, |(_, d)| clamp(long(&d, "fuel_capacity")))
    }
    fn insert(&self, host: &mut dyn ServerHost, _tx: &Transaction, i: usize, r: &ChemicalResource, amount: u32) -> u32 {
        let Some((field, id)) = FIELDS.get(i) else { return 0 };
        if r.0 != *id {
            return 0;
        }
        let Some((_, d)) = mb::master(host, self.at) else { return 0 };
        let moved = i64::from(amount).min((long(&d, "fuel_capacity") - long(&d, field)).max(0));
        if moved > 0 {
            self.add(host, field, moved);
        }
        moved as u32
    }
    fn extract(&self, _: &mut dyn ServerHost, _tx: &Transaction, _: usize, _: &ChemicalResource, _: u32) -> u32 {
        0
    }
}

struct Reactor;

#[mod_api::horizon_part]
impl BlockPart for Reactor {}

#[mod_api::horizon_part]
impl BlockEntityPart for Reactor {
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
            |id| matches!(id, Some(FRAME | PORT)),
            is_air,
            |_, _, found, nbt| {
                let n = found.interior.len() as i64;
                set_long(nbt, "fuel_capacity", n * FUEL_PER_BLOCK);
                set_long(nbt, "energy_capacity", n * ENERGY_PER_BLOCK);
            },
            |nbt| {
                let (d, t, e) = fuse(
                    long(nbt, "deuterium"),
                    long(nbt, "tritium"),
                    long(nbt, "energy"),
                    long(nbt, "energy_capacity") - long(nbt, "energy"),
                );
                set_long(nbt, "deuterium", d);
                set_long(nbt, "tritium", t);
                set_long(nbt, "energy", e);
            },
        );
    }
}

pub fn register(r: &mut dyn Registrar) {
    let built = r.block_entity_type("fusion_reactor_port").blocks(&[PORT]).build();
    let _ = r.block_entity_part(&built, Reactor);
    r.block_entity_capability(&CHEMICALS, &built, |_, at| Some(Box::new(Port { at })));
    r.block_entity_capability(&ENERGY, &built, |_, at| Some(Box::new(Port { at })));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuel_fuses_in_pairs_up_to_the_rate_and_the_room() {
        assert_eq!(fuse(100, 100, 0, 10_000_000), (90, 90, 200_000));
        assert_eq!(fuse(3, 100, 0, 10_000_000), (0, 97, 60_000));
        assert_eq!(fuse(100, 0, 5, 10_000_000), (100, 0, 5));
        // room for only 2 pairs
        assert_eq!(fuse(100, 100, 0, 40_000), (98, 98, 40_000));
    }
}
