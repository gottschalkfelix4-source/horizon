//! The Meka-Tool and the MekaSuit: items that hold energy. Each carries `mekanism:energy` (FE),
//! shows it as a bar, and is charged through the item energy capability by anything that
//! charges items. Stand-in: the tool mines like a fast netherite tool and the armor is
//! netherite-grade with no module system, no energy drain while mining, no energy-paid damage
//! absorption, no jetpack, night vision or other suit effects (needs: an item-used-on-block
//! hook, see ENGINE-GAPS.md). See README.
use std::sync::OnceLock;

use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, ClientHost, Component, ComponentId, ComponentTypesEvent,
    Compound, Host, ITEM_ENERGY, ItemBarEvent, ItemComponentEnergy, ItemKey, ItemPart,
    ItemStack, LevelSide, ModContainer, ModEnergyBuffer, PlayerRef, Registrar, ServerHost, Tag,
};

pub const TOOL: &str = "mekanism:meka_tool";
pub const SUIT: [&str; 4] = [
    "mekanism:mekasuit_helmet",
    "mekanism:mekasuit_bodyarmor",
    "mekanism:mekasuit_pants",
    "mekanism:mekasuit_boots",
];
pub const CHARGER: &str = "mekanism:charge_station";
/// FE the tool spends on a block, and the station's charge a tick per item.
pub const MINING_COST: i32 = 200;
pub const CHARGE_RATE: i32 = 20_000;
pub const STATION_CAPACITY: i32 = 1_000_000;
pub const ENERGY_ID: &str = "mekanism:energy";
/// FE the tool holds (16 MJ at 2.5 J per FE, as far as remembered) and each suit piece.
pub const TOOL_CAPACITY: u32 = 6_400_000;
pub const SUIT_CAPACITY: u32 = 6_400_000;

pub struct Energy;

impl Component for Energy {
    type Value = i32;
    fn id(&self) -> &str {
        ENERGY_ID
    }
    fn decode(&self, value: &Tag) -> Option<i32> {
        match value {
            Tag::Int(v) => Some(*v),
            _ => None,
        }
    }
    fn encode(&self, value: i32) -> Tag {
        Tag::Int(value)
    }
}

/// The width and colour of the energy bar for `energy` of `capacity`.
pub fn bar(energy: u32, capacity: u32) -> Option<(i32, u32)> {
    if capacity == 0 {
        return None;
    }
    let width = (13.0 * f64::from(energy.min(capacity)) / f64::from(capacity)).round() as i32;
    Some((width, 0x3F_C8_3F))
}

struct MekaItem;

#[mod_api::horizon_part]
impl ItemPart for MekaItem {
    fn item_bar(&self, _: &dyn ClientHost, event: &mut ItemBarEvent<'_>) {
        let energy = event.stack.get(&Energy).unwrap_or(0).max(0) as u32;
        *event.bar = bar(energy, TOOL_CAPACITY);
    }
}

static TOOL_KEY: OnceLock<ItemKey> = OnceLock::new();

/// What the tool holds after mining one block, or None if `stack` is no Meka-Tool.
pub fn after_mining(energy: i32) -> i32 {
    (energy - MINING_COST).max(0)
}

/// FE the station puts into one item holding `held` of `capacity`, with `available` in the station.
pub fn charge(held: i32, capacity: i32, available: i32) -> i32 {
    CHARGE_RATE.min(capacity.saturating_sub(held)).min(available).max(0)
}

/// The held Meka-Tool of `player` pays for the block it just broke.
pub fn pay_for_mining(host: &mut dyn ServerHost, player: PlayerRef) {
    let Some(tool) = TOOL_KEY.get() else { return };
    let Some(slot) = player.selected_slot(&*host) else { return };
    let Some(slots) = player.inventory_slots(&*host) else { return };
    let Some((_, mut stack)) = slots.into_iter().find(|(i, _)| *i as usize == slot) else { return };
    if stack.item() != *tool {
        return;
    }
    let left = after_mining(stack.get(&Energy).unwrap_or(0));
    if stack.set(&Energy, left).is_ok() {
        let _ = player.set_inventory_slot(host, slot as u32, Some(stack));
    }
}

struct Station;

#[mod_api::horizon_part]
impl BlockPart for Station {}

#[mod_api::horizon_part]
impl BlockEntityPart for Station {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == CHARGER).then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == CHARGER {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let mut items = event.entity.items.clone();
        let mut energy = match event.entity.nbt.get(&"energy".into()) {
            Some(Tag::Int(v)) => *v,
            _ => 0,
        };
        let before = energy;
        for slot in items.iter_mut().flatten() {
            let capacity = if slot.item().id(&*host) == TOOL { TOOL_CAPACITY } else { SUIT_CAPACITY } as i32;
            let known = slot.item().id(&*host) == TOOL || SUIT.contains(&slot.item().id(&*host).as_str());
            if !known {
                continue;
            }
            let held = slot.get(&Energy).unwrap_or(0);
            let put = charge(held, capacity, energy);
            if put > 0 && slot.set(&Energy, held + put).is_ok() {
                energy -= put;
            }
        }
        if energy == before && items == event.entity.items {
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        }
        event.entity.nbt.insert("energy".into(), Tag::Int(energy));
        event.entity.items = items;
    }
}

pub fn component_types(event: &mut ComponentTypesEvent<'_>) {
    event.types.push(ENERGY_ID.to_string());
}

pub fn register(r: &mut dyn Registrar) {
    let key = r.block_key(CHARGER);
    r.block_energy(
        &key,
        ModEnergyBuffer {
            capacity: STATION_CAPACITY,
            max_insert: 100_000,
            max_extract: 0,
            insert_faces: [true; 6],
            extract_faces: [false; 6],
        },
    );
    r.block_container(
        &key,
        ModContainer {
            slots: 5,
            faces: std::array::from_fn(|_| (0..5).collect()),
            inputs: (0..5).collect(),
            outputs: (0..5).collect(),
            menu: Some("minecraft:hopper".into()),
            slot_limit: 0,
        },
    );
    let built = r.block_entity_type("charge_station").blocks(&[CHARGER]).build();
    let _ = r.block_entity_part(&built, Station);
    let _ = TOOL_KEY.set(r.item_key(TOOL));
    for (id, capacity) in std::iter::once((TOOL, TOOL_CAPACITY)).chain(SUIT.iter().map(|s| (*s, SUIT_CAPACITY))) {
        let item = r.item_key(id);
        r.item_part(item, MekaItem);
        r.item_capability(&ITEM_ENERGY, id, move |host, at| {
            let stack: ItemStack = at.stack(host)?;
            let _ = &stack;
            let component = ComponentId::new(ENERGY_ID)?;
            Some(Box::new(ItemComponentEnergy::new(at.clone(), component, capacity, capacity, capacity)))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mining_and_charging_move_energy_within_the_limits() {
        assert_eq!(after_mining(1_000), 800);
        assert_eq!(after_mining(50), 0);
        assert_eq!(charge(0, 6_400_000, 1_000_000), CHARGE_RATE);
        assert_eq!(charge(6_399_990, 6_400_000, 1_000_000), 10);
        assert_eq!(charge(0, 6_400_000, 5), 5);
        assert_eq!(charge(0, 6_400_000, 0), 0);
    }

    #[test]
    fn the_bar_follows_the_charge() {
        assert_eq!(bar(0, 100).map(|b| b.0), Some(0));
        assert_eq!(bar(50, 100).map(|b| b.0), Some(7));
        assert_eq!(bar(500, 100).map(|b| b.0), Some(13));
        assert_eq!(bar(5, 0), None);
    }
}
