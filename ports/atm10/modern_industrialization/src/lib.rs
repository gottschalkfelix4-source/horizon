//! Modern Industrialization for Horizon: the Bronze Boiler, the machine the steam age runs on.
//! It burns furnace fuel, heats up slowly while it burns and cools while it doesn't, and boils
//! water into steam faster the hotter it is.
//!
//! Written from Modern Industrialization's observed behaviour and public description, never from
//! its code (docs/modpacks.md, "Licences"). The numbers (temperatures, rates, tank sizes) are
//! from memory and listed in README.md as facts to check against the NeoForge original.
//!
//! How it works: `modern_industrialization:bronze_boiler` has a block entity with a fuel slot
//! (a furnace's menu shows it: its flame is the fuel left, its arrow the temperature) and two
//! tanks the game keeps under `fluids` in its saved compound: water in (filled through the
//! `horizon:fluids` capability, by a pipe or a bucket), steam out (drained the same way). Each
//! tick the boiler burns, heats or cools, and turns water into steam.
use std::sync::OnceLock;

use mod_api::{
    BlockEntityPart, BlockEntityTickEvent, BlockPart, Compound, Hooks, ItemKey, ItemStack,
    ModContainer, ModFluidTanks, Registrar, RegistrationContext, ServerHost, Tag,
};

/// The fuel slot; the furnace menu's slots 0 and 2 are unused.
pub const FUEL: usize = 1;
/// The water tank and the steam tank.
pub const WATER: usize = 0;
pub const STEAM: usize = 1;
/// Each tank's size, in millibuckets.
pub const TANK: i32 = 16_000;
/// The water the boiler boils at (°C); below it, no steam.
pub const BOILING: u32 = 100;
/// The hottest the boiler gets (°C).
pub const MAX_TEMPERATURE: u32 = 1_100;
/// How much a tick of burning heats it, and a tick without fuel cools it.
pub const HEAT_PER_TICK: u32 = 1;
/// The steam (mB per tick) it makes at its hottest.
pub const MAX_STEAM_PER_TICK: u32 = 8;
/// A fuel's burn time is divided by this: a boiler burns through fuel faster than a furnace.
pub const BURN_DIVISOR: i32 = 1;

pub const STEAM_ID: &str = "modern_industrialization:steam";
pub const WATER_ID: &str = "minecraft:water";

/// The temperature after one tick, burning or not.
pub fn heat(temperature: u32, burning: bool) -> u32 {
    if burning {
        (temperature + HEAT_PER_TICK).min(MAX_TEMPERATURE)
    } else {
        temperature.saturating_sub(HEAT_PER_TICK)
    }
}

/// The steam the boiler could make this tick at `temperature`: none below boiling, then rising
/// in proportion to the heat until the full rate at its hottest.
pub fn steam_rate(temperature: u32) -> u32 {
    if temperature < BOILING {
        return 0;
    }
    MAX_STEAM_PER_TICK * temperature / MAX_TEMPERATURE
}

/// How much water becomes steam this tick: the rate, as far as the water and the steam tank's
/// room allow (one millibucket of water is one of steam).
pub fn boil(rate: u32, water: u32, steam_room: u32) -> u32 {
    rate.min(water).min(steam_room)
}

/// One tank as saved: its fluid and amount.
#[derive(Clone, Debug, PartialEq)]
pub struct Tank {
    pub fluid: Option<String>,
    pub amount: u32,
}

fn read_tanks(nbt: &Compound) -> [Tank; 2] {
    let mut tanks = [
        Tank {
            fluid: None,
            amount: 0,
        },
        Tank {
            fluid: None,
            amount: 0,
        },
    ];
    let Some(Tag::List(list)) = nbt.get(&"fluids".into()) else {
        return tanks;
    };
    for (tank, tag) in tanks.iter_mut().zip(list) {
        let Tag::Compound(map) = tag else { continue };
        if let Some(Tag::String(fluid)) = map.get(&"Fluid".into()) {
            let fluid = fluid.to_string_lossy();
            if fluid != "minecraft:empty" {
                tank.fluid = Some(fluid);
            }
        }
        if let Some(Tag::Int(amount)) = map.get(&"Amount".into()) {
            tank.amount = (*amount).max(0) as u32;
        }
    }
    tanks
}

fn write_tanks(nbt: &mut Compound, tanks: &[Tank; 2]) {
    let list = tanks
        .iter()
        .map(|tank| {
            let fluid = match (&tank.fluid, tank.amount) {
                (Some(fluid), 1..) => fluid.as_str(),
                _ => "minecraft:empty",
            };
            let map: Compound = [
                ("Fluid".into(), Tag::String(fluid.into())),
                ("Amount".into(), Tag::Int(tank.amount as i32)),
            ]
            .into();
            Tag::Compound(map)
        })
        .collect();
    nbt.insert("fluids".into(), Tag::List(list));
}

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(n)) => *n,
        _ => 0,
    }
}

/// The lava bucket and the bucket it leaves.
static BUCKETS: OnceLock<(ItemKey, ItemKey)> = OnceLock::new();

/// One tick of the boiler, on its saved compound and fuel slot. `burn_time` answers an item's
/// furnace burn time.
fn tick(
    nbt: &mut Compound,
    fuel: &mut Option<ItemStack>,
    burn_time: impl Fn(ItemKey) -> Option<i32>,
) -> (u32, i32, i32) {
    let mut tanks = read_tanks(nbt);
    let mut temperature = int(nbt, "temperature").max(0) as u32;
    let mut burn = int(nbt, "burn").max(0);
    let mut burn_total = int(nbt, "burn_total").max(0);

    let water = if tanks[WATER].fluid.as_deref() == Some(WATER_ID) {
        tanks[WATER].amount
    } else {
        0
    };
    let steam_room = match tanks[STEAM].fluid.as_deref() {
        None => TANK as u32,
        Some(STEAM_ID) => (TANK as u32).saturating_sub(tanks[STEAM].amount),
        Some(_) => 0,
    };
    // A new piece of fuel is lit only when the boiler has something to boil and room for it.
    if burn == 0
        && water > 0
        && steam_room > 0
        && let Some(stack) = fuel.as_ref()
        && let Some(time) = burn_time(stack.item()).filter(|&t| t > 0)
    {
        burn = (time / BURN_DIVISOR).max(1);
        burn_total = burn;
        let taken = fuel.take().expect("checked above");
        *fuel = match BUCKETS.get() {
            Some(&(lava, bucket)) if taken.item() == lava && taken.count() == 1 => {
                ItemStack::new(bucket, 1)
            }
            _ => taken.shrink(1),
        };
    }
    let burning = burn > 0;
    if burning {
        burn -= 1;
    }
    temperature = heat(temperature, burning);
    let made = boil(steam_rate(temperature), water, steam_room);
    if made > 0 {
        tanks[WATER].amount -= made;
        tanks[STEAM].fluid = Some(STEAM_ID.into());
        tanks[STEAM].amount += made;
        write_tanks(nbt, &tanks);
    }
    nbt.insert("temperature".into(), Tag::Int(temperature as i32));
    nbt.insert("burn".into(), Tag::Int(burn));
    nbt.insert("burn_total".into(), Tag::Int(burn_total));
    (temperature, burn, burn_total)
}

/// The boiler's block entity.
struct BronzeBoiler;

#[mod_api::horizon_part]
impl BlockPart for BronzeBoiler {}

#[mod_api::horizon_part]
impl BlockEntityPart for BronzeBoiler {
    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let entity = &mut event.entity;
        if entity.items.len() <= FUEL {
            return;
        }
        let mut fuel = entity.items[FUEL].take();
        let mut nbt = std::mem::take(&mut entity.nbt);
        let (temperature, burn, burn_total) =
            tick(&mut nbt, &mut fuel, |item| host.item_burn_time(item));
        entity.nbt = nbt;
        entity.items[FUEL] = fuel;
        // The furnace menu: flame = fuel left of the piece burning, arrow = temperature.
        let data = &mut entity.data;
        if data.len() < 4 {
            data.resize(4, 0);
        }
        data[0] = burn;
        data[1] = burn_total;
        data[2] = temperature as i32;
        data[3] = MAX_TEMPERATURE as i32;
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let _ = BUCKETS.set((
            r.item_key("minecraft:lava_bucket"),
            r.item_key("minecraft:bucket"),
        ));
        let block = r.block_key("modern_industrialization:bronze_boiler");
        r.block_container(
            &block,
            ModContainer {
                slots: 3,
                faces: std::array::from_fn(|_| vec![FUEL]),
                inputs: vec![FUEL],
                outputs: Vec::new(),
                menu: Some("minecraft:furnace".into()),
            },
        );
        // `ModFluidTanks` is non-exhaustive: built from its default, field by field.
        let mut tanks = ModFluidTanks::default();
        tanks.tanks = vec![TANK, TANK];
        tanks.faces = std::array::from_fn(|_| vec![WATER, STEAM]);
        tanks.inputs = vec![WATER];
        tanks.outputs = vec![STEAM];
        r.block_fluids(&block, tanks);
        let kind = r
            .block_entity_type("bronze_boiler")
            .blocks(&["bronze_boiler"])
            .build();
        let _ = r.block_entity_part(&kind, BronzeBoiler);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_boiler_heats_while_it_burns_and_cools_after() {
        let mut t = 0;
        for _ in 0..2_000 {
            t = heat(t, true);
        }
        assert_eq!(t, 1_100);
        for _ in 0..300 {
            t = heat(t, false);
        }
        assert_eq!(t, 800);
        assert_eq!(heat(0, false), 0);
    }

    #[test]
    fn steam_starts_at_boiling_and_reaches_full_rate_when_hottest() {
        assert_eq!(steam_rate(99), 0);
        assert_eq!(steam_rate(550), 4);
        assert_eq!(steam_rate(1_100), 8);
        // Water and the steam tank's room each cap a tick's steam.
        assert_eq!(boil(8, 3, 100), 3);
        assert_eq!(boil(8, 100, 5), 5);
    }

    #[test]
    fn saved_tanks_round_trip_and_water_becomes_steam() {
        let mut nbt = Compound::new();
        write_tanks(
            &mut nbt,
            &[
                Tank {
                    fluid: Some(WATER_ID.into()),
                    amount: 1_000,
                },
                Tank {
                    fluid: None,
                    amount: 0,
                },
            ],
        );
        nbt.insert("temperature".into(), Tag::Int(1_100));
        let mut fuel = None;
        // Hottest and no fuel: it cools by one and still boils 7 mB.
        let (t, burn, _) = tick(&mut nbt, &mut fuel, |_| None);
        assert_eq!((t, burn), (1_099, 0));
        let tanks = read_tanks(&nbt);
        assert_eq!(tanks[WATER].amount, 993);
        assert_eq!(tanks[STEAM].fluid.as_deref(), Some(STEAM_ID));
        assert_eq!(tanks[STEAM].amount, 7);
    }
}
