//! Import and export buses. A bus is a block joined to the network; every `BUS_PERIOD` ticks it
//! moves one item between the network's cells and the inventories (blocks with an item capability)
//! around it that are not part of the network. It needs power and a channel.
//!
//! The import bus takes the first item it finds (only the one named by its filter, when it has one);
//! the export bus puts out the item named by its filter (nothing without one). Right-click a bus with
//! an item to set its filter to that item, with an empty hand to clear it.
use mod_api::{
    Compound, Direction, Hand, HostTransaction, Identifier, InteractionResult, ItemPlace, ItemResource, ItemStack, ITEMS,
    LevelRef, ResourceHandler, ServerHost, Tag, Text, UseItemOnEvent, UseWithoutItemEvent,
};

use crate::rules::*;
use crate::terminal::net;
use crate::{id_at, network_extract, network_insert, offset};

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

fn resource_of(host: &dyn ServerHost, id: &str) -> Option<ItemResource> {
    let key = host.item_key(&Identifier::parse(id).ok()?)?;
    Some(ItemResource::of(&ItemStack::new(key, 1)?))
}

/// One operation of the bus at `pos`, run on the ticks that are a multiple of the period.
pub fn tick(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], device: Device, nbt: &Compound) {
    if level.game_time(&*host).unwrap_or(0) % BUS_PERIOD != 0 {
        return;
    }
    let n = net(host, level, pos);
    if !n.powered || !n.channel {
        return;
    }
    let filter = filter_of(nbt);
    for dir in Direction::ALL {
        let other = offset(pos, dir);
        // blocks of the network are not inventories to work on
        if id_at(&*host, level, other).as_deref().and_then(parse_block).is_some() {
            continue;
        }
        let Some(cap) = level.capability(host, &ITEMS, other, Some(dir.opposite())) else { continue };
        let done = match device {
            Device::ImportBus => import(host, level, &n.drives, &*cap, filter.as_deref()),
            _ => filter.as_deref().is_some_and(|f| export(host, level, &n.drives, &*cap, f)),
        };
        if done {
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

/// Sets the filter of the bus at `pos` to the held item (or clears it), and says so.
fn set_filter(host: &mut dyn ServerHost, pos: [i32; 3], player: u128, held: Option<&ItemStack>) -> Option<InteractionResult> {
    let level = host.level(host.dimension())?;
    let be = host.block_entity(level, pos)?;
    let filter = held.map(|h| h.item().id(&*host)).unwrap_or_default();
    let mut patch = Compound::new();
    patch.insert(FILTER.into(), Tag::String(filter.as_str().into()));
    let _ = be.merge_data(host, patch);
    if let Some(p) = host.player(player) {
        let msg = if filter.is_empty() { "Bus filter cleared".to_string() } else { format!("Bus filter: {filter}") };
        let _ = p.action_bar(host, Text::literal(&msg));
    }
    Some(InteractionResult::SuccessServer)
}

/// What the player holds in `hand`: the click's own stack, else a read of the hand.
fn held(host: &mut dyn ServerHost, click_stack: Option<&ItemStack>, player: u128, hand: Hand) -> Option<ItemStack> {
    click_stack.cloned().or_else(|| ItemPlace::PlayerHand { player, hand }.stack(host))
}

pub fn use_item_on(host: &mut dyn ServerHost, event: &mut UseItemOnEvent<'_>) -> Option<InteractionResult> {
    let click = event.click;
    let stack = held(&mut *host, click.stack, click.player, click.hand);
    set_filter(host, click.pos, click.player, stack.as_ref())
}

pub fn use_without_item(host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
    let click = event.click;
    // The game also runs this empty-hand use right after the item use of the same click, so an empty
    // hand alone must not clear the filter: sneaking with an empty hand does.
    let stack = held(&mut *host, click.stack, click.player, click.hand);
    if stack.is_none() {
        let sneaking = host
            .player(click.player)
            .and_then(|p| p.is_secondary_use_active(&*host))
            .unwrap_or(false);
        if !sneaking {
            return None;
        }
    }
    set_filter(host, click.pos, click.player, stack.as_ref())
}
