//! The Meka-Tool and the MekaSuit: items that hold energy. Each carries `mekanism:energy` (FE),
//! shows it as a bar, and is charged through the item energy capability by anything that
//! charges items. Stand-in: the tool mines like a fast netherite tool and the armor is
//! netherite-grade with no module system, no energy drain while mining, no energy-paid damage
//! absorption, no jetpack, night vision or other suit effects (needs: an item-used-on-block
//! hook, see ENGINE-GAPS.md). See README.
use mod_api::{
    ClientHost, Component, ComponentId, ComponentTypesEvent, ITEM_ENERGY, ItemBarEvent,
    ItemComponentEnergy, ItemPart, ItemStack, Registrar, Tag,
};

pub const TOOL: &str = "mekanism:meka_tool";
pub const SUIT: [&str; 4] = [
    "mekanism:mekasuit_helmet",
    "mekanism:mekasuit_bodyarmor",
    "mekanism:mekasuit_pants",
    "mekanism:mekasuit_boots",
];
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

pub fn component_types(event: &mut ComponentTypesEvent<'_>) {
    event.types.push(ENERGY_ID.to_string());
}

pub fn register(r: &mut dyn Registrar) {
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
    fn the_bar_follows_the_charge() {
        assert_eq!(bar(0, 100).map(|b| b.0), Some(0));
        assert_eq!(bar(50, 100).map(|b| b.0), Some(7));
        assert_eq!(bar(500, 100).map(|b| b.0), Some(13));
        assert_eq!(bar(5, 0), None);
    }
}
