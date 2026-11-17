//! PneumaticCraft: Repressurized for Horizon: pressure. The Air Compressor burns furnace fuel
//! into compressed air; air flows through Pressure Tubes from higher pressure to lower until
//! the pressures are equal; a block pushed past its danger pressure may explode, and past its
//! critical pressure it does.
//!
//! Written from PneumaticCraft's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). Volumes, rates and limits are from memory and listed in
//! README.md as facts to check against the NeoForge original.
//!
//! How it works: each pneumatic block has a block entity whose saved compound holds its air
//! (`air`, in mL above atmospheric pressure; pressure in bar is air / volume). Each tick a block
//! pushes air into every neighbouring pneumatic block at a lower pressure, so that the two end
//! at the same pressure (writing the neighbour's saved compound through its handle); the
//! compressor also adds air while it burns fuel; and a block over its limits rolls for an
//! explosion.
use std::sync::OnceLock;

use mod_api::{
    BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply, BlockKey, BlockPart, Compound,
    ExplosionInteraction, ExplosionSettings, Hooks, ItemKey, ItemStack, ModContainer, RandomSource,
    Registrar, RegistrationContext, ServerHost, Tag,
};

/// Tier-one pressures, in bar.
pub const DANGER_PRESSURE: f32 = 5.0;
pub const CRITICAL_PRESSURE: f32 = 7.0;
/// The lowest pressure a block holds (a vacuum).
pub const MIN_PRESSURE: f32 = -1.0;
/// Volumes in mL.
pub const COMPRESSOR_VOLUME: i32 = 5_000;
pub const TUBE_VOLUME: i32 = 1_000;
/// Air the compressor adds per tick of burning.
pub const COMPRESSOR_AIR_PER_TICK: i32 = 10;
/// Chance per tick, in millionths, of exploding just under critical pressure; it rises from 0
/// at danger pressure.
pub const DANGER_CHANCE_PPM: u32 = 5_000;
/// The explosion's radius.
pub const EXPLOSION_RADIUS: f32 = 1.0;
/// The compressor's fuel slot (the furnace menu's).
pub const FUEL: usize = 1;

/// A block's pressure in bar.
pub fn pressure(air: i32, volume: i32) -> f32 {
    air as f32 / volume as f32
}

/// The air that flows from block A to block B, both in mL and with their volumes, so that they
/// end at the same pressure; none when B's pressure is not lower. Air is conserved: what A
/// loses B gains, rounded so A keeps the odd millilitre and no flow ever overshoots (which
/// would send it back the next tick).
pub fn flow(air_a: i32, volume_a: i32, air_b: i32, volume_b: i32) -> i32 {
    let (a, va, b, vb) = (
        i64::from(air_a),
        i64::from(volume_a),
        i64::from(air_b),
        i64::from(volume_b),
    );
    // Pressure A > pressure B, compared without division.
    if a * vb <= b * va {
        return 0;
    }
    let n = (a + b) * va;
    let target_a = -((-n).div_euclid(va + vb));
    (a - target_a).max(0) as i32
}

/// Whether a block at `pressure` explodes this tick, given a roll in 0..1_000_000: never at or
/// below danger pressure, with a chance rising to `DANGER_CHANCE_PPM` towards critical, and
/// always above critical.
pub fn explodes(pressure: f32, roll: u32) -> bool {
    if pressure <= DANGER_PRESSURE {
        return false;
    }
    if pressure > CRITICAL_PRESSURE {
        return true;
    }
    let share = (pressure - DANGER_PRESSURE) / (CRITICAL_PRESSURE - DANGER_PRESSURE);
    roll < (share * DANGER_CHANCE_PPM as f32) as u32
}

const NEIGHBOURS: [[i32; 3]; 6] = [
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
    [-1, 0, 0],
    [1, 0, 0],
];

fn air_of(nbt: &Compound) -> i32 {
    match nbt.get(&"air".into()) {
        Some(Tag::Int(n)) => *n,
        _ => 0,
    }
}

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(n)) => *n,
        _ => 0,
    }
}

struct Keys {
    compressor: BlockKey,
    tube: BlockKey,
    lava_bucket: ItemKey,
    bucket: ItemKey,
}

static KEYS: OnceLock<Keys> = OnceLock::new();

/// The volume of a pneumatic block, none for anything else.
fn volume_of(keys: &Keys, block: BlockKey) -> Option<i32> {
    if block == keys.compressor {
        Some(COMPRESSOR_VOLUME)
    } else if block == keys.tube {
        Some(TUBE_VOLUME)
    } else {
        None
    }
}

/// One tick of a pneumatic block of `volume` holding `air`: share it with lower-pressure
/// neighbours, then roll for an explosion. Returns the air left and whether it exploded.
fn tick_air(host: &mut dyn ServerHost, pos: [i32; 3], volume: i32, mut air: i32) -> (i32, bool) {
    let Some(keys) = KEYS.get() else {
        return (air, false);
    };
    let Some(level) = host.level(host.dimension()) else {
        return (air, false);
    };
    for d in NEIGHBOURS {
        let next = [pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]];
        let Some(their_volume) = level.block(&*host, next).and_then(|b| volume_of(keys, b)) else {
            continue;
        };
        let Some(entity) = host.block_entity(level, next) else {
            continue;
        };
        let Some(data) = entity.data(&*host) else {
            continue;
        };
        let theirs = air_of(&data);
        let moved = flow(air, volume, theirs, their_volume);
        if moved == 0 {
            continue;
        }
        let update: Compound = [("air".into(), Tag::Int(theirs + moved))].into();
        if entity.merge_data(host, update) == Ok(true) {
            air -= moved;
        }
    }
    let pressure = pressure(air, volume);
    // Only a block over danger pressure draws from the level's random, so a quiet network
    // leaves the level's stream as vanilla's.
    let roll = if pressure > DANGER_PRESSURE {
        level.with_random(host, |random| random.next_int_bound(1_000_000).unwrap_or(0))
    } else {
        None
    };
    if let Some(roll) = roll
        && explodes(pressure, roll as u32)
    {
        let center = [
            f64::from(pos[0]) + 0.5,
            f64::from(pos[1]) + 0.5,
            f64::from(pos[2]) + 0.5,
        ];
        let blew = level.explode(
            host,
            None,
            center,
            EXPLOSION_RADIUS,
            false,
            ExplosionInteraction::Block,
            ExplosionSettings::default(),
        );
        return (air, blew.is_ok());
    }
    (air, false)
}

/// The Air Compressor: burns fuel, makes air, and shares it.
struct Compressor;

#[mod_api::horizon_part]
impl BlockPart for Compressor {}

#[mod_api::horizon_part]
impl BlockEntityPart for Compressor {
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let entity = &mut event.entity;
        let mut air = air_of(&entity.nbt);
        let mut burn = int(&entity.nbt, "burn").max(0);
        let mut burn_total = int(&entity.nbt, "burn_total").max(0);
        if burn == 0
            && entity.items.len() > FUEL
            && let Some(stack) = entity.items[FUEL].as_ref()
            && let Some(time) = host.item_burn_time(stack.item()).filter(|&t| t > 0)
        {
            burn = time;
            burn_total = time;
            let taken = entity.items[FUEL].take().expect("checked above");
            entity.items[FUEL] = match KEYS.get() {
                Some(keys) if taken.item() == keys.lava_bucket && taken.count() == 1 => {
                    ItemStack::new(keys.bucket, 1)
                }
                _ => taken.shrink(1),
            };
        }
        if burn > 0 {
            burn -= 1;
            air += COMPRESSOR_AIR_PER_TICK;
        }
        let (air, exploded) = tick_air(host, pos, COMPRESSOR_VOLUME, air);
        if exploded {
            return;
        }
        entity.nbt.insert("air".into(), Tag::Int(air));
        entity.nbt.insert("burn".into(), Tag::Int(burn));
        entity.nbt.insert("burn_total".into(), Tag::Int(burn_total));
        // The furnace menu: flame = fuel left, arrow = pressure up to critical (in 0.1 bar).
        let data = &mut entity.data;
        if data.len() < 4 {
            data.resize(4, 0);
        }
        data[0] = burn;
        data[1] = burn_total;
        data[2] = (pressure(air, COMPRESSOR_VOLUME) * 10.0).max(0.0) as i32;
        data[3] = (CRITICAL_PRESSURE * 10.0) as i32;
    }
}

/// A Pressure Tube: holds a little air and passes it on.
struct Tube;

#[mod_api::horizon_part]
impl BlockPart for Tube {}

#[mod_api::horizon_part]
impl BlockEntityPart for Tube {
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let before = air_of(&event.entity.nbt);
        let (air, exploded) = tick_air(host, event.pos, TUBE_VOLUME, before);
        if exploded || air == before {
            // Nothing of its own changed (what it pushed went through the neighbours' handles):
            // the tick leaves its chunk clean.
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        }
        event.entity.nbt.insert("air".into(), Tag::Int(air));
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let compressor = r.block_key("pneumaticcraft:air_compressor");
        let _ = KEYS.set(Keys {
            compressor,
            tube: r.block_key("pneumaticcraft:pressure_tube"),
            lava_bucket: r.item_key("minecraft:lava_bucket"),
            bucket: r.item_key("minecraft:bucket"),
        });
        r.block_container(
            &compressor,
            ModContainer {
                slots: 3,
                faces: std::array::from_fn(|_| vec![FUEL]),
                inputs: vec![FUEL],
                outputs: Vec::new(),
                menu: Some("minecraft:furnace".into()),
                slot_limit: 0,
            },
        );
        let kind = r
            .block_entity_type("air_compressor")
            .blocks(&["air_compressor"])
            .build();
        let _ = r.block_entity_part(&kind, Compressor);
        let kind = r
            .block_entity_type("pressure_tube")
            .blocks(&["pressure_tube"])
            .build();
        let _ = r.block_entity_part(&kind, Tube);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn air_flows_to_equal_pressure_and_is_conserved() {
        // A compressor at 4 bar (20 000 mL in 5 000) beside an empty tube (1 000 mL): the
        // tube takes its sixth of the 20 000 mL, the compressor keeping the odd millilitre.
        let moved = flow(20_000, COMPRESSOR_VOLUME, 0, TUBE_VOLUME);
        assert_eq!(moved, 3_333);
        let (a, b) = (20_000 - moved, moved);
        assert_eq!(a + b, 20_000);
        assert!((pressure(a, COMPRESSOR_VOLUME) - pressure(b, TUBE_VOLUME)).abs() < 0.01);
        // Equal or higher pressure downstream: nothing flows either way any more.
        assert_eq!(flow(a, COMPRESSOR_VOLUME, b, TUBE_VOLUME), 0);
        assert_eq!(flow(b, TUBE_VOLUME, a, COMPRESSOR_VOLUME), 0);
        assert_eq!(flow(1_000, TUBE_VOLUME, 5_000, COMPRESSOR_VOLUME), 0);
    }

    #[test]
    fn a_vacuum_pulls_air_in() {
        // A tube at -1 bar next to one at 0: air flows from the 0 bar tube into the vacuum.
        assert_eq!(flow(0, TUBE_VOLUME, -1_000, TUBE_VOLUME), 500);
    }

    #[test]
    fn danger_pressure_risks_and_critical_pressure_explodes() {
        assert!(!explodes(5.0, 0));
        assert!(!explodes(-1.0, 0));
        // Halfway to critical: half the top chance (2 500 ppm).
        assert!(explodes(6.0, 2_499));
        assert!(!explodes(6.0, 2_500));
        assert!(explodes(7.01, 999_999));
    }
}
