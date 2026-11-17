//! Import and export buses. A bus is a block joined to the network; every `BUS_PERIOD` ticks it
//! moves one item between the network's cells and the inventories (blocks with an item capability)
//! around it that are not part of the network. It needs power and a channel.
//!
//! The import bus takes the first item it finds (only the one named by its filter, when it has one);
//! the export bus puts out the item named by its filter (nothing without one). The filter is the item in the bus's first slot:
//! right-click the bus to open it and put the item there.
use mod_api::{
    Compound, Direction, HostTransaction, Identifier, ItemResource, ItemStack, ITEMS, LevelRef, ResourceHandler,
    ServerHost, Tag,
};

use crate::rules::*;
use crate::terminal::net;
use crate::{id_at, network_extract, network_insert, offset};

/// The upgrade card a bus takes in its slots 1 to 4.
pub const SPEED_CARD: &str = "ae2:speed_card";

/// How many items one operation moves with `cards` speed cards (0 to 4 count).
pub fn items_per_operation(cards: usize) -> usize {
    [1, 2, 4, 8, 16][cards.min(4)]
}

const FILTER: &str = "filter";

pub fn filter_of(nbt: &Compound) -> Option<String> {
    match nbt.get(&FILTER.into()) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()).filter(|s| !s.is_empty()),
        _ => None,
    }
}

fn plain(stack: &ItemStack) -> bool {
    stack.components().is_none_or(|c| c.as_nbt().is_empty())
}

pub(crate) fn resource_of(host: &dyn ServerHost, id: &str) -> Option<ItemResource> {
    let key = host.item_key(&Identifier::parse(id).ok()?)?;
    Some(ItemResource::of(&ItemStack::new(key, 1)?))
}

/// One operation of the bus at `pos`, run on the ticks that are a multiple of the period.
pub fn tick(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], device: Device, filter: Option<String>, cards: usize) {
    if level.game_time(&*host).unwrap_or(0) % BUS_PERIOD != 0 {
        return;
    }
    let n = net(host, level, pos);
    if !n.powered || !n.channel {
        return;
    }
    for dir in Direction::ALL {
        let other = offset(pos, dir);
        // blocks of the network are not inventories to work on
        if id_at(&*host, level, other).as_deref().and_then(parse_block).is_some() {
            continue;
        }
        let Some(cap) = level.capability(host, &ITEMS, other, Some(dir.opposite())) else { continue };
        let mut any = false;
        for _ in 0..items_per_operation(cards) {
            let done = match device {
                Device::ImportBus => import(host, level, &n.drives, &*cap, filter.as_deref()),
                _ => filter.as_deref().is_some_and(|f| export(host, level, &n.drives, &*cap, f)),
            };
            if !done {
                break;
            }
            any = true;
        }
        if any {
            break;
        }
    }
}

fn import(
    host: &mut dyn ServerHost,
    level: LevelRef,
    drives: &[[i32; 3]],
    cap: &dyn ResourceHandler<ItemResource>,
    filter: Option<&str>,
) -> bool {
    for i in 0..cap.size(host) {
        let Some(res) = cap.resource(host, i) else { continue };
        let Some(stack) = res.to_stack(1) else { continue };
        let id = stack.item().id(&*host);
        if !plain(&stack) || filter.is_some_and(|f| f != id) {
            continue;
        }
        let took = host.transaction(|host, tx| Ok::<u32, ()>(cap.extract(host, tx, i, &res, 1))).unwrap_or(0);
        if took == 0 {
            continue;
        }
        if network_insert(host, level, drives, &id, 1) == 1 {
            return true;
        }
        // the cells had no room: the item goes back where it came from
        let _ = host.transaction(|host, tx| Ok::<u32, ()>(cap.insert_any(host, tx, &res, 1)));
    }
    false
}

fn export(host: &mut dyn ServerHost, level: LevelRef, drives: &[[i32; 3]], cap: &dyn ResourceHandler<ItemResource>, id: &str) -> bool {
    let Some(res) = resource_of(&*host, id) else { return false };
    if network_extract(host, level, drives, id, 1) == 0 {
        return false;
    }
    let put = host.transaction(|host, tx| Ok::<u32, ()>(cap.insert_any(host, tx, &res, 1))).unwrap_or(0);
    if put == 0 {
        // the inventory was full: back into the network
        network_insert(host, level, drives, id, 1);
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_cards_double_the_items_per_operation_up_to_four() {
        assert_eq!(items_per_operation(0), 1);
        assert_eq!(items_per_operation(2), 4);
        assert_eq!(items_per_operation(4), 16);
        assert_eq!(items_per_operation(9), 16);
    }
}
