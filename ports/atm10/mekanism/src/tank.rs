//! The dynamic tank: a hollow box (3 to 18 per side) of dynamic tank casing and valves, empty
//! inside, holding 16 buckets (or 16 000 mB of a chemical) per interior block, of one fluid or
//! one chemical at a time. Every valve is a `FLUIDS` and a `mekanism:chemicals` capability for the
//! whole structure; the lowest valve keeps the contents. Valves re-check the structure every second.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityRef, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, BlockSide, Compound, FLUIDS, FluidResource, FluidStack, Host,
    LevelRef, LevelSide, Registrar, ResourceHandler, ServerHost, Tag, Transaction,
};

use crate::chemical::{CHEMICALS, ChemicalResource};
use crate::multiblock::{self, Pos};

pub const VALVE: &str = "mekanism:dynamic_valve";
pub const CASING: &str = "mekanism:dynamic_tank";
/// mB per interior block.
pub const PER_BLOCK: i64 = 16_000;

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

fn text(nbt: &Compound, key: &str) -> Option<String> {
    match nbt.get(&key.into()) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()).filter(|s| !s.is_empty()),
        _ => None,
    }
}

/// What a tank holds: nothing, a fluid or a chemical, by id.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Contents {
    pub kind: Option<Kind>,
    pub id: String,
    pub amount: i64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Fluid,
    Chemical,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Fluid => "fluid",
            Kind::Chemical => "chemical",
        }
    }
}

impl Contents {
    pub fn load(nbt: &Compound) -> Contents {
        let amount = long(nbt, "amount").max(0);
        let kind = match text(nbt, "kind").as_deref() {
            Some("fluid") => Some(Kind::Fluid),
            Some("chemical") => Some(Kind::Chemical),
            _ => None,
        };
        match kind {
            Some(k) if amount > 0 => Contents { kind: Some(k), id: text(nbt, "stored").unwrap_or_default(), amount },
            _ => Contents::default(),
        }
    }

    pub fn save(&self, nbt: &mut Compound) {
        set_long(nbt, "amount", self.amount);
        nbt.insert("kind".into(), Tag::String(self.kind.map_or("", Kind::name).into()));
        nbt.insert("stored".into(), Tag::String(self.id.as_str().into()));
    }

    /// Puts in up to `amount` of `id` of `kind`, with room for `capacity`: how much went in.
    pub fn fill(&mut self, kind: Kind, id: &str, amount: i64, capacity: i64) -> i64 {
        if self.amount > 0 && (self.kind != Some(kind) || self.id != id) {
            return 0;
        }
        let moved = amount.min(capacity - self.amount).max(0);
        if moved > 0 {
            self.kind = Some(kind);
            self.id = id.to_owned();
            self.amount += moved;
        }
        moved
    }

    /// Takes out up to `amount` of `id` of `kind`: how much came out.
    pub fn drain(&mut self, kind: Kind, id: &str, amount: i64) -> i64 {
        if self.amount == 0 || self.kind != Some(kind) || self.id != id {
            return 0;
        }
        let moved = amount.min(self.amount).max(0);
        self.amount -= moved;
        if self.amount == 0 {
            *self = Contents::default();
        }
        moved
    }
}

fn id_at(host: &dyn Host, level: LevelRef, p: Pos) -> Option<String> {
    level.block(host, p).map(|k| k.id(host))
}

fn is_air(id: Option<&str>) -> bool {
    matches!(id, None | Some("minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"))
}

fn structure(host: &dyn Host, level: LevelRef, pos: Pos) -> Option<multiblock::Found> {
    multiblock::find(
        pos,
        |p| matches!(id_at(host, level, p).as_deref(), Some(CASING | VALVE)),
        |p| is_air(id_at(host, level, p).as_deref()),
    )
}

/// A valve: its structure's master entity and data, while formed.
struct Valve {
    at: BlockSide,
}

impl Valve {
    fn master(&self, host: &mut dyn ServerHost) -> Option<(BlockEntityRef, Compound)> {
        let own = host.block_entity(self.at.level, self.at.pos)?.data(&*host)?;
        if long(&own, "formed") != 1 {
            return None;
        }
        let pos = [long(&own, "mx") as i32, long(&own, "my") as i32, long(&own, "mz") as i32];
        let entity = host.block_entity(self.at.level, pos)?;
        let data = entity.data(&*host)?;
        Some((entity, data))
    }

    fn put(&self, host: &mut dyn ServerHost, kind: Kind, id: &str, amount: u32) -> u32 {
        let Some((entity, d)) = self.master(host) else { return 0 };
        let mut c = Contents::load(&d);
        let moved = c.fill(kind, id, i64::from(amount), long(&d, "capacity"));
        if moved > 0 {
            let mut patch = Compound::new();
            c.save(&mut patch);
            let _ = entity.merge_data(host, patch);
        }
        moved as u32
    }

    fn take(&self, host: &mut dyn ServerHost, kind: Kind, id: &str, amount: u32) -> u32 {
        let Some((entity, d)) = self.master(host) else { return 0 };
        let mut c = Contents::load(&d);
        let moved = c.drain(kind, id, i64::from(amount));
        if moved > 0 {
            let mut patch = Compound::new();
            c.save(&mut patch);
            let _ = entity.merge_data(host, patch);
        }
        moved as u32
    }

    fn contents(&self, host: &mut dyn ServerHost) -> (Contents, i64) {
        self.master(host).map_or((Contents::default(), 0), |(_, d)| (Contents::load(&d), long(&d, "capacity")))
    }
}

impl ResourceHandler<FluidResource> for Valve {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        1
    }
    fn resource(&self, host: &mut dyn ServerHost, _: usize) -> Option<FluidResource> {
        let (c, _) = self.contents(host);
        if c.kind != Some(Kind::Fluid) {
            return None;
        }
        let key = host.fluid_key(&c.id)?;
        Some(FluidResource::of(&FluidStack::new(key, 1)))
    }
    fn amount(&self, host: &mut dyn ServerHost, _: usize) -> u32 {
        let (c, _) = self.contents(host);
        if c.kind == Some(Kind::Fluid) { c.amount.min(i64::from(u32::MAX)) as u32 } else { 0 }
    }
    fn capacity(&self, host: &mut dyn ServerHost, _: usize, _: &FluidResource) -> u32 {
        let (c, cap) = self.contents(host);
        if c.kind == Some(Kind::Chemical) { 0 } else { cap.min(i64::from(u32::MAX)) as u32 }
    }
    fn insert(&self, host: &mut dyn ServerHost, _tx: &Transaction, _: usize, r: &FluidResource, amount: u32) -> u32 {
        let Some(key) = r.to_stack(1).key() else { return 0 };
        let id = key.id(&*host);
        self.put(host, Kind::Fluid, &id, amount)
    }
    fn extract(&self, host: &mut dyn ServerHost, _tx: &Transaction, _: usize, r: &FluidResource, amount: u32) -> u32 {
        let Some(key) = r.to_stack(1).key() else { return 0 };
        let id = key.id(&*host);
        self.take(host, Kind::Fluid, &id, amount)
    }
}

impl ResourceHandler<ChemicalResource> for Valve {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        1
    }
    fn resource(&self, host: &mut dyn ServerHost, _: usize) -> Option<ChemicalResource> {
        let (c, _) = self.contents(host);
        (c.kind == Some(Kind::Chemical)).then(|| ChemicalResource(c.id))
    }
    fn amount(&self, host: &mut dyn ServerHost, _: usize) -> u32 {
        let (c, _) = self.contents(host);
        if c.kind == Some(Kind::Chemical) { c.amount.min(i64::from(u32::MAX)) as u32 } else { 0 }
    }
    fn capacity(&self, host: &mut dyn ServerHost, _: usize, _: &ChemicalResource) -> u32 {
        let (c, cap) = self.contents(host);
        if c.kind == Some(Kind::Fluid) { 0 } else { cap.min(i64::from(u32::MAX)) as u32 }
    }
    fn insert(&self, host: &mut dyn ServerHost, _tx: &Transaction, _: usize, r: &ChemicalResource, amount: u32) -> u32 {
        self.put(host, Kind::Chemical, &r.0, amount)
    }
    fn extract(&self, host: &mut dyn ServerHost, _tx: &Transaction, _: usize, r: &ChemicalResource, amount: u32) -> u32 {
        self.take(host, Kind::Chemical, &r.0, amount)
    }
}

/// One part for the dynamic valve's block entity.
struct TankValve;

#[mod_api::horizon_part]
impl BlockPart for TankValve {}

#[mod_api::horizon_part]
impl BlockEntityPart for TankValve {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == VALVE).then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == VALVE {
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
                    let master = found
                        .shell
                        .iter()
                        .copied()
                        .find(|p| id_at(&*host, level, *p).as_deref() == Some(VALVE))
                        .unwrap_or(pos);
                    set_long(&mut nbt, "formed", 1);
                    set_long(&mut nbt, "mx", i64::from(master[0]));
                    set_long(&mut nbt, "my", i64::from(master[1]));
                    set_long(&mut nbt, "mz", i64::from(master[2]));
                    if master == pos {
                        let capacity = found.interior.len() as i64 * PER_BLOCK;
                        set_long(&mut nbt, "capacity", capacity);
                        let mut c = Contents::load(&nbt);
                        c.amount = c.amount.min(capacity);
                        c.save(&mut nbt);
                    }
                }
                None => {
                    set_long(&mut nbt, "formed", 0);
                    set_long(&mut nbt, "capacity", 0);
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

/// Registers the valve's block entity and its capabilities.
pub fn register(r: &mut dyn Registrar) {
    let built = r.block_entity_type("dynamic_valve").blocks(&[VALVE]).build();
    let _ = r.block_entity_part(&built, TankValve);
    r.block_entity_capability(&FLUIDS, &built, |_, at| Some(Box::new(Valve { at })));
    r.block_entity_capability(&CHEMICALS, &built, |_, at| Some(Box::new(Valve { at })));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tank_holds_one_thing_up_to_its_capacity_and_empties_to_nothing() {
        let mut c = Contents::default();
        assert_eq!(c.fill(Kind::Fluid, "minecraft:water", 10_000, 16_000), 10_000);
        assert_eq!(c.fill(Kind::Chemical, "mekanism:hydrogen", 100, 16_000), 0);
        assert_eq!(c.fill(Kind::Fluid, "minecraft:lava", 100, 16_000), 0);
        assert_eq!(c.fill(Kind::Fluid, "minecraft:water", 10_000, 16_000), 6_000);
        assert_eq!(c.drain(Kind::Chemical, "minecraft:water", 5), 0);
        assert_eq!(c.drain(Kind::Fluid, "minecraft:water", 99_999), 16_000);
        assert_eq!(c, Contents::default());
        assert_eq!(c.fill(Kind::Chemical, "mekanism:hydrogen", 1, 16_000), 1);
    }

    #[test]
    fn contents_survive_saving() {
        let mut c = Contents::default();
        c.fill(Kind::Chemical, "mekanism:oxygen", 250, 16_000);
        let mut nbt = Compound::new();
        c.save(&mut nbt);
        assert_eq!(Contents::load(&nbt), c);
    }
}
