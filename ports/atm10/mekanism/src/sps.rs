//! The supercritical phase shifter: a hollow box (3 to 18 per side) of SPS casing and ports,
//! empty inside. Polonium (tank 0) goes in through `mekanism:chemicals` and energy through
//! `ENERGY`; antimatter (tank 1) comes out through `mekanism:chemicals`. Stand-in model: every
//! tick up to 10 mB of polonium turn into the same of antimatter for 2 000 FE a mB; no coil
//! ring, no 7x7x7 size, no efficiency by size. See README.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockPart,
    BlockSide, Compound, ENERGY, EnergyHandler, Host, LevelSide, Registrar, ResourceHandler,
    ServerHost, Transaction,
};

use crate::chemical::{CHEMICALS, ChemicalResource};
use crate::mb::{self, is_air, long, set_long};

pub const POLONIUM: &str = "mekanism:polonium";
pub const ANTIMATTER: &str = "mekanism:antimatter";
pub const PORT: &str = "mekanism:sps_port";
pub const CASING: &str = "mekanism:sps_casing";
pub const MB_PER_TICK: i64 = 10;
pub const FE_PER_MB: i64 = 2_000;
pub const PER_BLOCK: i64 = 1_000;
pub const ENERGY_PER_BLOCK: i64 = 100_000;

/// One tick: (polonium, antimatter, energy) afterwards.
pub fn shift(polonium: i64, antimatter: i64, energy: i64, anti_room: i64) -> (i64, i64, i64) {
    let n = polonium.min(MB_PER_TICK).min(energy / FE_PER_MB).min(anti_room).max(0);
    (polonium - n, antimatter + n, energy - n * FE_PER_MB)
}

fn clamp(v: i64) -> u32 {
    v.clamp(0, i64::from(u32::MAX)) as u32
}

const FIELDS: [(&str, &str); 2] = [("polonium", POLONIUM), ("antimatter", ANTIMATTER)];

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
    fn insert(&self, host: &mut dyn ServerHost, _tx: &Transaction, amount: u32) -> u32 {
        let Some((_, d)) = mb::master(host, self.at) else { return 0 };
        let moved = i64::from(amount).min((long(&d, "energy_capacity") - long(&d, "energy")).max(0));
        if moved > 0 {
            self.add(host, "energy", moved);
        }
        moved as u32
    }
    fn extract(&self, _: &mut dyn ServerHost, _tx: &Transaction, _: u32) -> u32 {
        0
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
        mb::master(host, self.at).map_or(0, |(_, d)| clamp(long(&d, "capacity")))
    }
    fn insert(&self, host: &mut dyn ServerHost, _tx: &Transaction, i: usize, r: &ChemicalResource, amount: u32) -> u32 {
        // only the polonium tank takes anything in
        if i != 0 || r.0 != POLONIUM {
            return 0;
        }
        let Some((_, d)) = mb::master(host, self.at) else { return 0 };
        let moved = i64::from(amount).min((long(&d, "capacity") - long(&d, "polonium")).max(0));
        if moved > 0 {
            self.add(host, "polonium", moved);
        }
        moved as u32
    }
    fn extract(&self, host: &mut dyn ServerHost, _tx: &Transaction, i: usize, r: &ChemicalResource, amount: u32) -> u32 {
        if i != 1 || r.0 != ANTIMATTER {
            return 0;
        }
        let Some((_, d)) = mb::master(host, self.at) else { return 0 };
        let moved = i64::from(amount).min(long(&d, "antimatter")).max(0);
        if moved > 0 {
            self.add(host, "antimatter", -moved);
        }
        moved as u32
    }
}

struct Shifter;

#[mod_api::horizon_part]
impl BlockPart for Shifter {}

#[mod_api::horizon_part]
impl BlockEntityPart for Shifter {
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
            is_air,
            |_, _, found, nbt| {
                let n = found.interior.len() as i64;
                set_long(nbt, "capacity", n * PER_BLOCK);
                set_long(nbt, "energy_capacity", n * ENERGY_PER_BLOCK);
            },
            |nbt| {
                let (p, a, e) = shift(
                    long(nbt, "polonium"),
                    long(nbt, "antimatter"),
                    long(nbt, "energy"),
                    long(nbt, "capacity") - long(nbt, "antimatter"),
                );
                set_long(nbt, "polonium", p);
                set_long(nbt, "antimatter", a);
                set_long(nbt, "energy", e);
            },
        );
    }
}

pub fn register(r: &mut dyn Registrar) {
    let built = r.block_entity_type("sps_port").blocks(&[PORT]).build();
    let _ = r.block_entity_part(&built, Shifter);
    r.block_entity_capability(&CHEMICALS, &built, |_, at| Some(Box::new(Port { at })));
    r.block_entity_capability(&ENERGY, &built, |_, at| Some(Box::new(Port { at })));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polonium_shifts_at_the_rate_the_energy_and_room_allow() {
        assert_eq!(shift(100, 0, 1_000_000, 1_000), (90, 10, 980_000));
        // energy for 3 mB only
        assert_eq!(shift(100, 0, 6_500, 1_000), (97, 3, 500));
        assert_eq!(shift(4, 0, 1_000_000, 1_000), (0, 4, 992_000));
        assert_eq!(shift(100, 995, 1_000_000, 5), (95, 1_000, 990_000));
    }
}
