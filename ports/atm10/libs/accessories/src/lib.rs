//! Accessory slots for Horizon ports: what Curios does for NeoForge mods, as a crate.
//!
//! A *slot type* (`ring`, `necklace`, `charm`...) has a size, an order in the screen and an
//! optional cosmetic row. Mods declare them in data (`data/<ns>/curios/slots/<name>.json`) and
//! say which entities carry which in `data/<ns>/curios/entities/<name>.json`, as Curios'
//! public documentation describes. An item fits a slot when it is in the item tag
//! `curios:<slot>` (or `curios:curio`, which fits every slot).
//!
//! Each player's worn items live in one attachment, `accessories:inventory`, saved with the
//! player and copied on death. Stacks are kept as item NBT (`ItemStack::to_nbt`), because an
//! attachment's codec runs without a host.
//!
//! Written from Curios' public documentation, not from its code. Not here yet: the screen,
//! per-item tick and equip callbacks, and drops on death (README.md).

pub mod data;
pub mod inventory;

pub use data::{EntityRule, SlotType, SlotTypes};
pub use inventory::{Inventory, Row};

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, Compound, Holder, Host,
    ItemStack, ServerHost,
};

/// The worn items of one player.
pub static INVENTORY: AttachmentType<Inventory> = AttachmentType {
    namespace: "accessories",
    name: "inventory",
    default: Inventory::default,
    codec: Some(AttachmentCodec { encode: encode, decode: decode }),
    copy_on_death: true,
    sync: false,
};

fn encode(inv: &Inventory) -> Result<Compound, AttachmentError> {
    Ok(inv.to_nbt())
}

fn decode(data: &Compound) -> Result<Inventory, AttachmentError> {
    Inventory::from_nbt(data).ok_or(AttachmentError::Decode)
}

/// Whether `stack` may be worn in slot type `slot`: its item is in `curios:<slot>` or
/// `curios:curio`. False while tags are unavailable.
pub fn fits(host: &dyn Host, slot: &str, stack: &ItemStack) -> bool {
    let tags = host.tags();
    [slot, "curio"].into_iter().any(|name| {
        mod_api::Identifier::from_parts("curios", name)
            .ok()
            .and_then(|id| tags.key(&id))
            .and_then(|key| tags.is_in_tag(stack.item(), key))
            .unwrap_or(false)
    })
}

/// Why a stack could not be put in a slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquipError {
    /// The player has no such slot type, or no such index in it.
    NoSlot,
    /// The item is not tagged for the slot.
    WrongItem,
    /// The slot holds something already.
    Occupied,
    /// The game could not give the stack as NBT (ids not loaded).
    Unavailable,
}

/// Puts one item into `slot[index]` of `player`'s accessories. `types` is what the player's
/// entity type gets (see [`SlotTypes::for_entity`]), which sizes the rows.
pub fn equip(
    host: &mut dyn ServerHost,
    types: &SlotTypes,
    entity_type: &str,
    player: u128,
    slot: &str,
    index: usize,
    stack: ItemStack,
    cosmetic: bool,
) -> Result<(), EquipError> {
    if !fits(host, slot, &stack) {
        return Err(EquipError::WrongItem);
    }
    let nbt = stack.to_nbt(host).ok_or(EquipError::Unavailable)?;
    let holder = Holder::Player(player);
    let mut inv = INVENTORY.get(host.attachments(), holder).map_err(|_| EquipError::Unavailable)?;
    inv.fit(types, entity_type);
    inv.put(slot, index, cosmetic, nbt)?;
    INVENTORY.set(host.attachments(), holder, inv).map_err(|_| EquipError::Unavailable)
}

/// Takes the item out of `slot[index]`, if any.
pub fn unequip(
    host: &mut dyn ServerHost,
    player: u128,
    slot: &str,
    index: usize,
    cosmetic: bool,
) -> Option<ItemStack> {
    let holder = Holder::Player(player);
    let mut inv = INVENTORY.get(host.attachments(), holder).ok()?;
    let nbt = inv.take(slot, index, cosmetic)?;
    INVENTORY.set(host.attachments(), holder, inv).ok()?;
    ItemStack::from_nbt(host, &nbt)
}

/// Every worn item of `player` (cosmetics excluded), by slot type and index.
pub fn worn(host: &mut dyn ServerHost, player: u128) -> Vec<(String, usize, ItemStack)> {
    let Ok(inv) = INVENTORY.get(host.attachments(), Holder::Player(player)) else {
        return Vec::new();
    };
    inv.items(false)
        .filter_map(|(slot, i, nbt)| Some((slot.to_owned(), i, ItemStack::from_nbt(host, nbt)?)))
        .collect()
}
