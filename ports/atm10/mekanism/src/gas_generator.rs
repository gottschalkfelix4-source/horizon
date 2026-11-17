//! The gas-burning generator: burns hydrogen or ethylene from `mekanism:chemicals` (one tank of
//! 18 000 mB) into energy, which it gives to the energy blocks beside it and which cables draw.
//! It lives in the Mekanism port, not in Generators', because a chemical capability is typed by
//! this crate. Stand-in numbers (40 FE a mB of hydrogen, 1 600 of ethylene, up to 10 mB a
//! tick), see README.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, BlockSide, Compound, Direction, ENERGY, Host, LevelRef,
    HostTransaction, LevelSide, ModEnergyBuffer, Registrar, ResourceHandler, ServerHost, Tag, Transaction,
};

use crate::chemical::{CHEMICALS, ChemicalResource};

pub const BLOCK: &str = "mekanism:gas_burning_generator";
pub const CAPACITY_MB: i32 = 18_000;
pub const BUFFER: i32 = 100_000;
pub const RATE: i32 = 2_000;
pub const MB_PER_TICK: i32 = 10;

/// FE one mB of this gas gives.
pub fn fe_per_mb(chemical: &str) -> Option<i32> {
    match chemical {
        "mekanism:hydrogen" => Some(40),
        "mekanism:ethylene" => Some(1_600),
        _ => None,
    }
}

/// One tick: (gas left, energy now) for `gas` mB of `chemical` and `energy` in a buffer with `room` left.
pub fn burn(chemical: &str, gas: i32, energy: i32, room: i32) -> (i32, i32) {
    let Some(per) = fe_per_mb(chemical) else { return (gas, energy) };
    let n = gas.min(MB_PER_TICK).min(room / per).max(0);
    (gas - n, energy + n * per)
}

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

fn text(nbt: &Compound, key: &str) -> Option<String> {
    match nbt.get(&key.into()) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()).filter(|s| !s.is_empty()),
        _ => None,
    }
}

fn offset(pos: [i32; 3], dir: Direction) -> [i32; 3] {
    let n = dir.normal();
    [pos[0] + n[0], pos[1] + n[1], pos[2] + n[2]]
}

struct Tank {
    at: BlockSide,
}

impl ResourceHandler<ChemicalResource> for Tank {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        1
    }
    fn resource(&self, host: &mut dyn ServerHost, _: usize) -> Option<ChemicalResource> {
        let d = host.block_entity(self.at.level, self.at.pos)?.data(&*host)?;
        (int(&d, "gas") > 0).then(|| ChemicalResource(text(&d, "kind").unwrap_or_default()))
    }
    fn amount(&self, host: &mut dyn ServerHost, _: usize) -> u32 {
        host.block_entity(self.at.level, self.at.pos)
            .and_then(|e| e.data(&*host))
            .map_or(0, |d| int(&d, "gas").max(0) as u32)
    }
    fn capacity(&self, _: &mut dyn ServerHost, _: usize, r: &ChemicalResource) -> u32 {
        if fe_per_mb(&r.0).is_some() { CAPACITY_MB as u32 } else { 0 }
    }
    fn insert(&self, host: &mut dyn ServerHost, _tx: &Transaction, _: usize, r: &ChemicalResource, amount: u32) -> u32 {
        if fe_per_mb(&r.0).is_none() {
            return 0;
        }
        let Some(entity) = host.block_entity(self.at.level, self.at.pos) else { return 0 };
        let Some(d) = entity.data(&*host) else { return 0 };
        let held = int(&d, "gas");
        // one gas at a time
        if held > 0 && text(&d, "kind").as_deref() != Some(r.0.as_str()) {
            return 0;
        }
        let put = (amount as i32).min(CAPACITY_MB - held).max(0);
        if put > 0 {
            let mut patch = Compound::new();
            patch.insert("gas".into(), Tag::Int(held + put));
            patch.insert("kind".into(), Tag::String(r.0.as_str().into()));
            let _ = entity.merge_data(host, patch);
        }
        put as u32
    }
    fn extract(&self, _: &mut dyn ServerHost, _tx: &Transaction, _: usize, _: &ChemicalResource, _: u32) -> u32 {
        0
    }
}

fn give(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], available: i32) -> i32 {
    let mut left = available.min(RATE).max(0) as u32;
    let mut given = 0u32;
    for dir in Direction::ALL {
        if left == 0 {
            break;
        }
        let Some(cap) = level.capability(host, &ENERGY, offset(pos, dir), Some(dir.opposite())) else { continue };
        let moved = host.transaction(|host, tx| Ok::<u32, ()>(cap.insert(host, tx, left))).unwrap_or(0);
        left -= moved.min(left);
        given += moved;
    }
    given as i32
}

struct Generator;

#[mod_api::horizon_part]
impl BlockPart for Generator {}

#[mod_api::horizon_part]
impl BlockEntityPart for Generator {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == BLOCK).then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == BLOCK {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let Some(level) = host.level(host.dimension()) else { return };
        let mut nbt = event.entity.nbt.clone();
        let kind = text(&nbt, "kind").unwrap_or_default();
        let (gas, mut energy) = burn(&kind, int(&nbt, "gas"), int(&nbt, "energy"), BUFFER - int(&nbt, "energy"));
        energy -= give(host, level, event.pos, energy);
        nbt.insert("gas".into(), Tag::Int(gas));
        nbt.insert("energy".into(), Tag::Int(energy));
        if nbt == event.entity.nbt {
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        }
        event.entity.nbt = nbt;
    }
}

pub fn register(r: &mut dyn Registrar) {
    let key = r.block_key(BLOCK);
    r.block_energy(
        &key,
        ModEnergyBuffer {
            capacity: BUFFER,
            max_insert: 0,
            max_extract: RATE,
            insert_faces: [false; 6],
            extract_faces: [true; 6],
        },
    );
    let built = r.block_entity_type("gas_burning_generator").blocks(&[BLOCK]).build();
    let _ = r.block_entity_part(&built, Generator);
    r.block_entity_capability(&CHEMICALS, &built, |_, at| Some(Box::new(Tank { at })));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gas_burns_into_energy_up_to_the_rate_and_the_room() {
        assert_eq!(burn("mekanism:hydrogen", 100, 0, 1_000), (90, 400));
        assert_eq!(burn("mekanism:ethylene", 100, 0, 3_300), (98, 3_200));
        assert_eq!(burn("mekanism:oxygen", 100, 0, 1_000), (100, 0));
        assert_eq!(burn("mekanism:hydrogen", 3, 5, 1_000), (0, 125));
    }
}
