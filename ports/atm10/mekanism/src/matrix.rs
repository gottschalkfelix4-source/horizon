//! The induction matrix: a box of induction casing and ports with induction cells (capacity) and
//! providers (rate) inside. Every port is an `ENERGY` capability for the whole structure; the
//! lowest port is its master and keeps the stored energy (`energy`, a long), the capacity and the
//! rate. Ports re-check the structure every second.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, BlockSide, Compound, ENERGY, EnergyHandler, Host,
    LevelRef, LevelSide, Registrar, ServerHost, Tag, Transaction,
};

use crate::multiblock::{self, Pos};
use crate::rules::{Tier, fe};

pub const PORT: &str = "mekanism:induction_port";
pub const CASING: &str = "mekanism:induction_casing";

pub fn cell_id(t: Tier) -> String {
    format!("mekanism:{}_induction_cell", t.name())
}

pub fn provider_id(t: Tier) -> String {
    format!("mekanism:{}_induction_provider", t.name())
}

/// A cell's capacity in FE (1, 8, 64, 512 GJ at 2.5 J per FE), as a long.
pub fn cell_capacity(t: Tier) -> i64 {
    let joules: i64 = match t {
        Tier::Basic => 1_000_000_000,
        Tier::Advanced => 8_000_000_000,
        Tier::Elite => 64_000_000_000,
        Tier::Ultimate => 512_000_000_000,
    };
    (joules as f64 / 2.5) as i64
}

/// A provider's rate in FE per tick.
pub fn provider_rate(t: Tier) -> i64 {
    i64::from(fe(match t {
        Tier::Basic => 256_000,
        Tier::Advanced => 1_024_000,
        Tier::Elite => 4_096_000,
        Tier::Ultimate => 16_384_000,
    }))
}

/// What a block id is in a matrix.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Casing,
    Port,
    Cell(Tier),
    Provider(Tier),
}

pub fn role(id: &str) -> Option<Role> {
    match id {
        CASING => return Some(Role::Casing),
        PORT => return Some(Role::Port),
        _ => {}
    }
    let name = id.strip_prefix("mekanism:")?;
    if let Some(t) = name.strip_suffix("_induction_cell") {
        return Tier::from_name(t).map(Role::Cell);
    }
    name.strip_suffix("_induction_provider").and_then(Tier::from_name).map(Role::Provider)
}

fn long(nbt: &Compound, key: &str) -> i64 {
    match nbt.get(&key.into()) {
        Some(Tag::Long(v)) => *v,
        Some(Tag::Int(v)) => i64::from(*v),
        _ => 0,
    }
}

fn set_long(nbt: &mut Compound, key: &str, v: i64) {
    nbt.insert(key.into(), Tag::Long(v));
}

fn id_at(host: &dyn Host, level: LevelRef, p: Pos) -> Option<String> {
    level.block(host, p).map(|k| k.id(host))
}

/// The structure `pos` (a port) is part of, if it is a valid matrix.
fn structure(host: &dyn Host, level: LevelRef, pos: Pos) -> Option<multiblock::Found> {
    multiblock::find(
        pos,
        |p| matches!(id_at(host, level, p).as_deref().and_then(role), Some(Role::Casing | Role::Port)),
        |p| match id_at(host, level, p).as_deref() {
            None | Some("minecraft:air") | Some("minecraft:cave_air") | Some("minecraft:void_air") => true,
            Some(id) => matches!(role(id), Some(Role::Cell(_) | Role::Provider(_))),
        },
    )
}

/// The capacity and rate of a structure, and its lowest port.
fn totals(host: &dyn Host, level: LevelRef, found: &multiblock::Found) -> (i64, i64, Pos) {
    let (mut capacity, mut rate) = (0i64, 0i64);
    for &p in &found.interior {
        match id_at(host, level, p).as_deref().and_then(role) {
            Some(Role::Cell(t)) => capacity += cell_capacity(t),
            Some(Role::Provider(t)) => rate += provider_rate(t),
            _ => {}
        }
    }
    let master = found
        .shell
        .iter()
        .copied()
        .find(|p| id_at(host, level, *p).as_deref() == Some(PORT))
        .unwrap_or(found.min);
    (capacity, rate, master)
}

/// A matrix port's energy: the whole structure's, kept by its master port.
struct MatrixEnergy {
    at: BlockSide,
}

impl MatrixEnergy {
    /// The master's handle and data, when this port is part of a formed matrix.
    fn master(&self, host: &mut dyn ServerHost) -> Option<(mod_api::BlockEntityRef, Compound)> {
        let own = host.block_entity(self.at.level, self.at.pos)?.data(&*host)?;
        if long(&own, "formed") != 1 {
            return None;
        }
        let pos = [long(&own, "mx") as i32, long(&own, "my") as i32, long(&own, "mz") as i32];
        let entity = host.block_entity(self.at.level, pos)?;
        let data = entity.data(&*host)?;
        Some((entity, data))
    }
}

fn clamp(v: i64) -> u32 {
    v.clamp(0, i64::from(u32::MAX)) as u32
}

impl EnergyHandler for MatrixEnergy {
    fn amount(&self, host: &mut dyn ServerHost) -> u32 {
        self.master(host).map_or(0, |(_, d)| clamp(long(&d, "energy")))
    }

    fn capacity(&self, host: &mut dyn ServerHost) -> u32 {
        self.master(host).map_or(0, |(_, d)| clamp(long(&d, "capacity")))
    }

    fn insert(&self, host: &mut dyn ServerHost, _tx: &Transaction, amount: u32) -> u32 {
        let Some((entity, d)) = self.master(host) else { return 0 };
        let room = (long(&d, "capacity") - long(&d, "energy")).max(0);
        let moved = i64::from(amount).min(room).min(long(&d, "rate"));
        if moved > 0 {
            let mut patch = Compound::new();
            set_long(&mut patch, "energy", long(&d, "energy") + moved);
            let _ = entity.merge_data(host, patch);
        }
        moved as u32
    }

    fn extract(&self, host: &mut dyn ServerHost, _tx: &Transaction, amount: u32) -> u32 {
        let Some((entity, d)) = self.master(host) else { return 0 };
        let moved = i64::from(amount).min(long(&d, "energy")).min(long(&d, "rate")).max(0);
        if moved > 0 {
            let mut patch = Compound::new();
            set_long(&mut patch, "energy", long(&d, "energy") - moved);
            let _ = entity.merge_data(host, patch);
        }
        moved as u32
    }
}

/// One part for the induction port's block entity.
struct MatrixPort;

#[mod_api::horizon_part]
impl BlockPart for MatrixPort {}

#[mod_api::horizon_part]
impl BlockEntityPart for MatrixPort {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == PORT).then(|| {
            let mut nbt = Compound::new();
            set_long(&mut nbt, "energy", 0);
            nbt
        })
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == PORT {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let mut nbt = event.entity.nbt.clone();
        let age = long(&nbt, "age") + 1;
        set_long(&mut nbt, "age", age % 20);
        if age % 20 == 1 {
            let Some(level) = host.level(host.dimension()) else { return };
            match structure(&*host, level, pos) {
                Some(found) => {
                    let (capacity, rate, master) = totals(&*host, level, &found);
                    set_long(&mut nbt, "formed", 1);
                    set_long(&mut nbt, "mx", i64::from(master[0]));
                    set_long(&mut nbt, "my", i64::from(master[1]));
                    set_long(&mut nbt, "mz", i64::from(master[2]));
                    if master == pos {
                        set_long(&mut nbt, "capacity", capacity);
                        set_long(&mut nbt, "rate", rate);
                        let held = long(&nbt, "energy").min(capacity);
                        set_long(&mut nbt, "energy", held);
                    }
                }
                None => {
                    set_long(&mut nbt, "formed", 0);
                    set_long(&mut nbt, "capacity", 0);
                    set_long(&mut nbt, "rate", 0);
                }
            }
        }
        if nbt == event.entity.nbt {
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        }
        event.entity.nbt = nbt;
    }
}

/// Registers the port's block entity and its energy capability.
pub fn register(r: &mut dyn Registrar) {
    let built = r.block_entity_type("induction_port").blocks(&[PORT]).build();
    let _ = r.block_entity_part(&built, MatrixPort);
    r.block_entity_capability(&ENERGY, &built, |_, at| Some(Box::new(MatrixEnergy { at })));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_and_totals_follow_the_block_ids() {
        assert_eq!(role("mekanism:induction_port"), Some(Role::Port));
        assert_eq!(role("mekanism:elite_induction_cell"), Some(Role::Cell(Tier::Elite)));
        assert_eq!(role("mekanism:basic_induction_provider"), Some(Role::Provider(Tier::Basic)));
        assert_eq!(role("mekanism:crusher"), None);
        assert_eq!(cell_capacity(Tier::Basic), 400_000_000);
        assert_eq!(cell_capacity(Tier::Advanced), 8 * cell_capacity(Tier::Basic));
        assert_eq!(provider_rate(Tier::Basic), 102_400);
    }
}
