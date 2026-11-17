//! Wireless access point and wireless terminal. Sneak-use the terminal item on an access point to link
//! it (the point's position is saved in the item's `minecraft:custom_data` under `ae2_link`); using
//! it in the air then opens the network's terminal view from anywhere within `RANGE` blocks of that
//! point, as long as the point is on a powered network with a channel.
use mod_api::{
    Compound, HostSide, HostTransaction, InteractionResult, ItemPart, ItemPlace, ItemStack, Tag, Text,
    UseItemEvent, UseOnEvent,
};

use crate::rules::*;
use crate::{id_at, terminal};

/// How far from its access point a wireless terminal works, in blocks.
pub const RANGE: f64 = 16.0;

pub const ITEM: &str = "ae2:wireless_terminal";

fn int(c: &Compound, k: &str) -> Option<i32> {
    match c.get(&k.into()) {
        Some(Tag::Int(v)) => Some(*v),
        _ => None,
    }
}

/// The access point a terminal is linked to.
pub fn link_of(stack: &ItemStack) -> Option<[i32; 3]> {
    let Some(Tag::Compound(data)) = stack.components().and_then(|c| c.as_nbt().get(&"minecraft:custom_data".into())) else {
        return None;
    };
    let Some(Tag::Compound(l)) = data.get(&"ae2_link".into()) else { return None };
    Some([int(l, "x")?, int(l, "y")?, int(l, "z")?])
}

pub fn set_link(stack: &mut ItemStack, pos: [i32; 3]) {
    let mut l = Compound::new();
    for (k, v) in ["x", "y", "z"].into_iter().zip(pos) {
        l.insert(k.into(), Tag::Int(v));
    }
    let mut data = Compound::new();
    data.insert("ae2_link".into(), Tag::Compound(l));
    let mut patch = Compound::new();
    patch.insert("minecraft:custom_data".into(), Tag::Compound(data));
    let _ = stack.apply_components(&patch);
}

/// Whether `from` is within the range of an access point at `ap` (the point's centre).
pub fn in_range(from: [i32; 3], ap: [i32; 3]) -> bool {
    let d: f64 = (0..3).map(|i| ((from[i] - ap[i]) as f64).powi(2)).sum();
    d <= RANGE * RANGE
}

fn tell(host: &mut dyn mod_api::ServerHost, player: u128, msg: &str) {
    if let Some(p) = host.player(player) {
        let _ = p.action_bar(host, Text::literal(msg));
    }
}

pub struct Terminal;

#[mod_api::horizon_part]
impl ItemPart for Terminal {
    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else { return None };
        let level = host.level(host.dimension())?;
        let is_point = id_at(&*host, level, event.pos).as_deref().and_then(parse_block)
            == Some(Kind::Device(Device::WirelessAccessPoint));
        let sneaking = host.player(event.player).and_then(|p| p.is_secondary_use_active(&*host)).unwrap_or(false);
        if !is_point || !sneaking {
            return None;
        }
        let pos = event.pos;
        let place = ItemPlace::PlayerHand { player: event.player, hand: event.hand };
        let done = host.transaction(|host, tx| if place.edit(host, tx, |s| set_link(s, pos)) { Ok(()) } else { Err(()) });
        tell(host, event.player, if done.is_ok() { "Wireless terminal linked" } else { "Could not link the terminal" });
        Some(InteractionResult::SuccessServer)
    }

    fn use_item(&self, host: HostSide, event: &mut UseItemEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else { return None };
        let Some(ap) = link_of(event.stack) else {
            tell(host, event.player, "Sneak-use the terminal on a wireless access point to link it");
            return Some(InteractionResult::Fail);
        };
        let level = host.level(host.dimension())?;
        if id_at(&*host, level, ap).as_deref().and_then(parse_block) != Some(Kind::Device(Device::WirelessAccessPoint)) {
            tell(host, event.player, "The linked access point is gone or not loaded");
            return Some(InteractionResult::Fail);
        }
        let here = host.player(event.player)?.entity().block_position(&*host)?;
        if !in_range(here, ap) {
            tell(host, event.player, "Out of range of the access point");
            return Some(InteractionResult::Fail);
        }
        terminal::open(host, event.player, ap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_is_sixteen_blocks() {
        assert!(in_range([0, 0, 0], [16, 0, 0]));
        assert!(!in_range([0, 0, 0], [17, 0, 0]));
        assert!(in_range([10, 10, 5], [0, 0, 0]));
        assert!(!in_range([10, 10, 10], [0, 0, 0]));
    }
}
