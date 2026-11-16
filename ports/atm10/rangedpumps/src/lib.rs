//! Ranged Pumps for Horizon: a pump that drains the fluid sources in an area below it into its
//! tank, for energy.
//!
//! Written from Ranged Pumps' observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What is ported, and what a check against the NeoForge original
//! still has to confirm, is in README.md.
//!
//! Every [`PERIOD`] ticks, when it holds [`COST`] energy and its tank can take a bucket, the pump
//! looks through the square of [`RADIUS`] blocks around the column below it, [`DEPTH`] layers
//! deep ([`scan`] gives the order), for a source block of the fluid it already holds (or any, when
//! empty) and takes it. Pipes and hoppers' fluid equivalents draw from the tank.
pub mod scan;

use std::sync::OnceLock;

use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Compound, Hooks, Host, HostTransaction, LevelRef, LevelSide, ModEnergyBuffer,
    ModFluidTanks, Registrar, RegistrationContext, ServerHost, Tag,
};

pub const RADIUS: i32 = 8;
pub const DEPTH: i32 = 32;
pub const PERIOD: i32 = 10;
/// Energy per source block taken.
pub const COST: i32 = 1_000;
/// Energy the pump holds, and the most it takes in a tick.
pub const CAPACITY: i32 = 100_000;
pub const MAX_INSERT: i32 = 2_000;
/// The tank: 16 buckets, in millibuckets.
pub const TANK: i32 = 16_000;
/// A source block is a bucket.
pub const BUCKET: i32 = 1_000;
/// Positions looked at in one operation.
pub const LOOKS_PER_OP: usize = 256;

static ORDER: OnceLock<Vec<(i32, i32)>> = OnceLock::new();

fn order() -> &'static [(i32, i32)] {
    ORDER.get_or_init(|| scan::layer_order(RADIUS))
}

/// Whether the tank (`held` of `fluid`, millibuckets) can take another bucket of `incoming`.
pub fn tank_takes(fluid: Option<&str>, held: i32, incoming: &str) -> bool {
    held + BUCKET <= TANK && fluid.is_none_or(|f| f == incoming || held == 0)
}

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

/// The first tank of the saved `fluids` list: its fluid and amount.
fn tank(nbt: &Compound) -> (Option<String>, i32) {
    if let Some(Tag::List(list)) = nbt.get(&"fluids".into()) {
        if let Some(t) = list.first() {
            let fluid = match t.get("Fluid") {
                Some(Tag::String(f)) => Some(f.to_string_lossy()).filter(|f| f != "minecraft:empty"),
                _ => None,
            };
            let amount = match t.get("Amount") {
                Some(Tag::Int(a)) => (*a).max(0),
                _ => 0,
            };
            return if amount == 0 { (None, 0) } else { (fluid, amount) };
        }
    }
    (None, 0)
}

fn set_tank(nbt: &mut Compound, fluid: &str, amount: i32) {
    let t = Tag::compound([("Fluid", Tag::String(fluid.into())), ("Amount", Tag::Int(amount))]);
    nbt.insert("fluids".into(), Tag::List(vec![t]));
}

fn operate(host: &mut dyn ServerHost, level: LevelRef, event: &mut BlockEntityTickEvent<'_>) -> bool {
    let energy = int(&event.entity.nbt, "energy");
    let (held_fluid, held) = tank(&event.entity.nbt);
    if energy < COST || held + BUCKET > TANK {
        return false;
    }
    let order = order();
    let total = scan::len(order, DEPTH);
    let start = int(&event.entity.nbt, "cursor").max(0) as usize % total;
    for k in 0..LOOKS_PER_OP.min(total) {
        let index = (start + k) % total;
        let Some(o) = scan::offset(order, DEPTH, index) else { continue };
        let pos = [event.pos[0] + o[0], event.pos[1] + o[1], event.pos[2] + o[2]];
        if !level.is_loaded(&*host, pos) {
            continue;
        }
        let Some(state) = level.fluid_state(&*host, pos) else { continue };
        if state.is_empty() || !state.is_source() {
            continue;
        }
        let id = state.fluid.id(&*host);
        if !tank_takes(held_fluid.as_deref(), held, &id) {
            continue;
        }
        let taken = host
            .transaction(|host, tx| Ok::<_, ()>(level.pickup_fluid(host, pos, None, tx)))
            .ok()
            .flatten();
        let Some(stack) = taken else { continue };
        let got = stack.amount.min(BUCKET as u32) as i32;
        if got == 0 {
            continue;
        }
        let nbt = &mut event.entity.nbt;
        set_tank(nbt, &id, held + got);
        nbt.insert("energy".into(), Tag::Int(energy - COST));
        nbt.insert("cursor".into(), Tag::Int(index as i32));
        return true;
    }
    // Nothing found in this stretch: carry on from where it ended next time.
    event.entity.nbt.insert("cursor".into(), Tag::Int(((start + LOOKS_PER_OP) % total) as i32));
    true
}

struct Pump;

#[mod_api::horizon_part]
impl BlockPart for Pump {}

#[mod_api::horizon_part]
impl BlockEntityPart for Pump {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == "rangedpumps:ranged_pump").then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == "rangedpumps:ranged_pump" {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let phase = (int(&event.entity.nbt, "phase") + 1) % PERIOD;
        if phase != 0 {
            event.entity.nbt.insert("phase".into(), Tag::Int(phase));
            return;
        }
        let Some(level) = host.level(host.dimension()) else { return };
        if !operate(host, level, event) {
            *event.reply = BlockEntityTickReply::Unchanged;
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let key = r.block_key("rangedpumps:ranged_pump");
        r.block_part(key, Pump);
        r.block_energy(&key, ModEnergyBuffer {
            capacity: CAPACITY,
            max_insert: MAX_INSERT,
            max_extract: 0,
            insert_faces: [true; 6],
            extract_faces: [false; 6],
        });
        let mut tanks = ModFluidTanks::default();
        tanks.tanks = vec![TANK];
        tanks.faces = std::array::from_fn(|_| vec![0]);
        tanks.inputs = Vec::new();
        tanks.outputs = vec![0];
        r.block_fluids(&key, tanks);
        let kind = r.block_entity_type("ranged_pump").blocks(&["rangedpumps:ranged_pump"]).build();
        let _ = r.block_entity_part(&kind, Pump);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tank_takes_a_bucket_of_its_own_fluid_or_any_when_empty_until_full() {
        assert!(tank_takes(None, 0, "minecraft:lava"));
        assert!(tank_takes(Some("minecraft:water"), 3000, "minecraft:water"));
        assert!(!tank_takes(Some("minecraft:water"), 3000, "minecraft:lava"));
        assert!(tank_takes(Some("minecraft:water"), 15_000, "minecraft:water"));
        assert!(!tank_takes(Some("minecraft:water"), 15_001, "minecraft:water"));
    }

    #[test]
    fn a_saved_tank_reads_back() {
        let mut nbt = Compound::new();
        set_tank(&mut nbt, "minecraft:lava", 4000);
        assert_eq!(tank(&nbt), (Some("minecraft:lava".into()), 4000));
        assert_eq!(tank(&Compound::new()), (None, 0));
    }
}
