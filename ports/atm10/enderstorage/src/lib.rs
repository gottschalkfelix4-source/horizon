//! EnderStorage for Horizon: Ender Chests and Ender Tanks whose contents are shared by every
//! block of the same frequency, anywhere in the world.
//!
//! Written from EnderStorage's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and what a check against the NeoForge
//! original still has to confirm, is in README.md.
//!
//! A frequency is three colours (the three buttons on the lid, left to right as seen from the
//! front), white by default, and an owner: a chest made personal with a diamond on its latch
//! belongs to the player who did it, opens only for them, and shares its contents only with that
//! player's other chests of the same colours. A dye on a button sets that button's colour.
//!
//! The contents live in the world's shared store (`store.rs`); each block entity holds a copy that
//! hoppers and the vanilla chest menu use, kept in step every tick. An Ender Pouch carries a
//! frequency (taken from a chest by sneak-using the pouch on it) and opens that chest's contents
//! from anywhere, in a chest view whose clicks this mod carries out (`pouch.rs`).
use std::sync::{Mutex, OnceLock};

use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockKey, BlockPart, Compound, Hand, Hooks, Host, HostSide, HostTransaction,
    InteractionResult, ItemKey, ItemPart, ItemPlace, ItemResource, ItemStack, LevelRef,
    MenuClickedEvent, MenuClosedEvent, ModContainer, ModFluidTanks, PreRemoveSideEffectsEvent,
    Registrar, RegistrationContext, ServerHost, ServerTickEvent, Tag, Text, UseItemEvent,
    UseItemOnEvent, UseWithoutItemEvent,
};

pub mod pouch;
pub mod store;

use store::{Items, Store, Sync, Tank};

/// Slots of an Ender Chest (EnderStorage's default `item storage size` of 1: three rows).
pub const CHEST_SLOTS: usize = 27;
/// An Ender Tank's capacity in millibuckets (16 buckets).
pub const TANK_CAPACITY: i32 = 16_000;

/// The dye colours in `DyeColor` order: a button's colour is its index here.
pub const DYES: [&str; 16] = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray",
    "cyan", "purple", "blue", "brown", "green", "red", "black",
];

/// A frequency: the three button colours, left to right, and the owner of a personal storage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Frequency {
    pub colours: [u8; 3],
    pub owner: Option<u128>,
}

impl Frequency {
    /// The shared store's key: chests and tanks of the same frequency are separate storages.
    pub fn key(&self) -> String {
        let [a, b, c] = self.colours;
        match self.owner {
            Some(owner) => format!("{owner:032x}/{a}-{b}-{c}"),
            None => format!("global/{a}-{b}-{c}"),
        }
    }

    pub fn load(nbt: &Compound) -> Self {
        let mut freq = Frequency::default();
        if let Some(Tag::Compound(f)) = nbt.get(&"frequency".into()) {
            for (i, key) in ["left", "middle", "right"].iter().enumerate() {
                if let Some(Tag::Byte(c)) = f.get(&(*key).into()) {
                    freq.colours[i] = (*c as u8).min(15);
                }
            }
            if let Some(Tag::IntArray(o)) = f.get(&"owner".into()) {
                if o.len() == 4 {
                    let bits = o.iter().fold(0u128, |acc, w| (acc << 32) | u128::from(*w as u32));
                    freq.owner = Some(bits);
                }
            }
        }
        freq
    }

    pub fn tag(&self) -> Tag {
        let mut f = Compound::new();
        for (i, key) in ["left", "middle", "right"].iter().enumerate() {
            f.insert((*key).into(), Tag::Byte(self.colours[i] as i8));
        }
        if let Some(owner) = self.owner {
            let words = [96, 64, 32, 0].map(|s| (owner >> s) as u32 as i32).to_vec();
            f.insert("owner".into(), Tag::IntArray(words));
        }
        Tag::Compound(f)
    }

    pub fn save(&self, nbt: &mut Compound) {
        nbt.insert("frequency".into(), self.tag());
    }
}

/// Where on an Ender Chest a click landed, for the dye and diamond rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    /// One of the three lid buttons, 0 the leftmost seen from the front.
    Button(usize),
    /// The latch on the front.
    Latch,
    /// Anywhere else.
    Body,
}

/// The front's normal of a block facing `facing` (the side the latch is on).
fn normal(facing: &str) -> [f64; 3] {
    match facing {
        "north" => [0.0, 0.0, -1.0],
        "south" => [0.0, 0.0, 1.0],
        "west" => [-1.0, 0.0, 0.0],
        _ => [1.0, 0.0, 0.0],
    }
}

/// Which spot `local` (the click inside the block, 0..1 on each axis) hits on a chest facing
/// `facing`, clicked on face `face`. The buttons sit side by side across the middle of the lid,
/// each a sixth of the block wide; the latch is the middle of the front's upper half.
pub fn spot(local: [f64; 3], face: [i32; 3], facing: &str) -> Spot {
    let n = normal(facing);
    let d = [local[0] - 0.5, local[1] - 0.5, local[2] - 0.5];
    // Right as seen by a viewer looking at the front, and depth from the front edge.
    let across = d[0] * n[2] - d[2] * n[0] + 0.5;
    let depth = 0.5 - (d[0] * n[0] + d[2] * n[2]);
    if face == [0, 1, 0] {
        if (0.25..0.75).contains(&across) && (0.3125..0.6875).contains(&depth) {
            return Spot::Button((((across - 0.25) * 6.0) as usize).min(2));
        }
        return Spot::Body;
    }
    let front = [n[0] as i32, 0, n[2] as i32];
    if face == front && (0.4375..0.5625).contains(&across) && (0.4375..0.8125).contains(&local[1]) {
        return Spot::Latch;
    }
    Spot::Body
}

/// What a click with `item` on `spot` does to the frequency `freq` of a block, for `player`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edit {
    /// Set button `button` to colour `colour`, using up one dye.
    Colour { button: usize, colour: u8 },
    /// Make the storage `player`'s, using up one diamond.
    Own,
    /// Nothing to change.
    None,
}

pub fn edit(freq: &Frequency, spot: Spot, dye: Option<u8>, diamond: bool, player: u128) -> Edit {
    match (spot, dye) {
        (Spot::Button(button), Some(colour)) if freq.colours[button] != colour => {
            Edit::Colour { button, colour }
        }
        (Spot::Latch, _) if diamond && freq.owner.is_none() => {
            let _ = player;
            Edit::Own
        }
        _ => Edit::None,
    }
}

/// Whether `player` may open a storage of `freq`.
pub fn may_open(freq: &Frequency, player: u128) -> bool {
    freq.owner.is_none_or(|owner| owner == player)
}

/// Keys known once registration ran.
struct Keys {
    chest: BlockKey,
    tank: BlockKey,
    dyes: [ItemKey; 16],
    diamond: ItemKey,
    pouch: ItemKey,
}

static KEYS: OnceLock<Keys> = OnceLock::new();

/// The shared chest inventories and tanks of the open world.
pub static CHESTS: Mutex<Store<Items>> = Mutex::new(Store::new("chests.nbt"));
pub static TANKS: Mutex<Store<Tank>> = Mutex::new(Store::new("tanks.nbt"));

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn dye_colour(item: ItemKey) -> Option<u8> {
    KEYS.get()?.dyes.iter().position(|d| *d == item).map(|i| i as u8)
}

fn is_chest(id: &str) -> bool {
    id == "enderstorage:ender_chest"
}

fn is_tank(id: &str) -> bool {
    id == "enderstorage:ender_tank"
}

/// Take one of the stack in `hand` from the player (a creative player keeps it).
fn use_up_one(host: &mut dyn ServerHost, player: u128, hand: Hand, stack: &ItemStack) {
    let place = ItemPlace::PlayerHand { player, hand };
    let one = ItemResource::of(stack);
    let _ = host.transaction(|host, tx| {
        if place.extract(host, tx, &one, 1) == 1 { Ok(()) } else { Err(()) }
    });
}

fn level_here(host: &dyn ServerHost) -> Option<LevelRef> {
    host.level(host.dimension())
}

/// The frequency saved in the block entity at `pos`.
fn frequency_at(host: &dyn ServerHost, pos: [i32; 3]) -> Option<Frequency> {
    let level = level_here(host)?;
    let entity = host.block_entity(level, pos)?;
    Some(Frequency::load(&entity.data(host)?))
}

/// Store a new frequency on the block entity at `pos`; its copy is pulled from the new storage
/// on its next tick.
fn set_frequency_at(host: &mut dyn ServerHost, pos: [i32; 3], freq: Frequency) -> bool {
    let Some(level) = level_here(host) else {
        return false;
    };
    let Some(entity) = host.block_entity(level, pos) else {
        return false;
    };
    let mut data = Compound::new();
    freq.save(&mut data);
    store::save_rev(&mut data, None);
    entity.merge_data(host, data).unwrap_or(false)
}

fn tell(host: &mut dyn ServerHost, player: u128, key: &str) {
    if let Some(p) = host.player(player) {
        let _ = p.action_bar(host, Text::translate(key, Vec::new()));
    }
}

/// The two blocks: dye and diamond clicks, opening, and the owner lock.
struct Storage;

#[mod_api::horizon_part]
impl BlockPart for Storage {
    fn use_item_on(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseItemOnEvent<'_>,
    ) -> Option<InteractionResult> {
        let keys = KEYS.get()?;
        let click = event.click;
        let stack = click.stack?;
        let pos = click.pos;
        let freq = frequency_at(host, pos)?;
        if stack.item() == keys.pouch {
            // The pouch takes the frequency; `pouch.rs` handles it as the item's own use.
            return None;
        }
        let local = [0, 1, 2].map(|i| click.hit.point[i] - f64::from(pos[i]));
        let level = level_here(host)?;
        let facing = level
            .block_property(host, pos, "facing")
            .unwrap_or_else(|| "north".into());
        let spot = spot(local, click.hit.face, &facing);
        let diamond = stack.item() == keys.diamond;
        match edit(&freq, spot, dye_colour(stack.item()), diamond, click.player) {
            Edit::Colour { button, colour } => {
                if !may_open(&freq, click.player) {
                    tell(host, click.player, "enderstorage.not_owner");
                    return Some(InteractionResult::Fail);
                }
                let mut new = freq;
                new.colours[button] = colour;
                if set_frequency_at(host, pos, new) {
                    use_up_one(host, click.player, click.hand, stack);
                }
                Some(InteractionResult::SuccessServer)
            }
            Edit::Own => {
                let new = Frequency { owner: Some(click.player), ..freq };
                if set_frequency_at(host, pos, new) {
                    use_up_one(host, click.player, click.hand, stack);
                }
                Some(InteractionResult::SuccessServer)
            }
            Edit::None => {
                if level.block(host, pos) == Some(keys.tank) {
                    return use_tank(host, event);
                }
                None
            }
        }
    }

    fn use_without_item(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseWithoutItemEvent<'_>,
    ) -> Option<InteractionResult> {
        let keys = KEYS.get()?;
        let click = event.click;
        let level = level_here(host)?;
        if level.block(host, click.pos) != Some(keys.chest) {
            return None;
        }
        let freq = frequency_at(host, click.pos)?;
        if !may_open(&freq, click.player) {
            tell(host, click.player, "enderstorage.not_owner");
            return Some(InteractionResult::Fail);
        }
        let entity = host.block_entity(level, click.pos)?;
        let player = host.player(click.player)?;
        let _ = player.open_menu(host, entity);
        Some(InteractionResult::Consume)
    }
}

/// A bucket or other fluid container on an Ender Tank: fill or empty it, as on any tank.
fn use_tank(host: &mut dyn ServerHost, event: &UseItemOnEvent<'_>) -> Option<InteractionResult> {
    let click = event.click;
    let freq = frequency_at(host, click.pos)?;
    if !may_open(&freq, click.player) {
        tell(host, click.player, "enderstorage.not_owner");
        return Some(InteractionResult::Fail);
    }
    let level = level_here(host)?;
    let tank = level.capability(host, &mod_api::FLUIDS, click.pos, None)?;
    let player = host.player(click.player)?;
    let moved = mod_api::fluid_util::interact_with_handler(
        host,
        player,
        click.hand,
        Some(click.pos),
        &tank,
    );
    moved.then_some(InteractionResult::SuccessServer)
}

/// The block entity: a frequency, a revision, and the copy of the storage.
struct Entity;

#[mod_api::horizon_part]
impl BlockPart for Entity {}

#[mod_api::horizon_part]
impl BlockEntityPart for Entity {
    fn block_entity(&self, _: mod_api::LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (is_chest(event.block) || is_tank(event.block)).then(|| {
            let mut nbt = Compound::new();
            Frequency::default().save(&mut nbt);
            store::save_rev(&mut nbt, None);
            nbt
        })
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if is_chest(event.block) || is_tank(event.block) {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let Some(keys) = KEYS.get() else { return };
        let Some(level) = level_here(host) else { return };
        let block = level.block(host, event.pos);
        let freq = Frequency::load(&event.entity.nbt);
        let mut rev = store::load_rev(&event.entity.nbt);
        let before = rev;
        let outcome = if block == Some(keys.chest) {
            let mut items = event.entity.items.clone();
            items.resize(CHEST_SLOTS, None);
            let mut chests = lock(&CHESTS);
            chests.open(host);
            let shared = chests.get(&freq.key(), || vec![None; CHEST_SLOTS]);
            let outcome = store::reconcile(&mut items, &mut rev, shared);
            if outcome == Sync::Pushed {
                chests.mark_dirty();
            }
            if outcome == (Sync::Pulled { changed: true }) || event.entity.items.len() != CHEST_SLOTS {
                event.entity.items = items;
            }
            outcome
        } else if block == Some(keys.tank) {
            let mut tank = store::first_tank(&event.entity.nbt);
            let mut tanks = lock(&TANKS);
            tanks.open(host);
            let shared = tanks.get(&freq.key(), || (None, 0));
            let outcome = store::reconcile(&mut tank, &mut rev, shared);
            if outcome == Sync::Pushed {
                tanks.mark_dirty();
            }
            if outcome == (Sync::Pulled { changed: true }) {
                store::set_first_tank(&mut event.entity.nbt, &tank);
            }
            outcome
        } else {
            return;
        };
        if outcome == Sync::Same || (rev == before && outcome == (Sync::Pulled { changed: false })) {
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        }
        store::save_rev(&mut event.entity.nbt, rev);
    }

    /// Breaking the block drops none of the storage: it stays with its frequency.
    fn pre_remove_side_effects(
        &self,
        _: &mut dyn mod_api::LevelRandomHost,
        event: &mut PreRemoveSideEffectsEvent<'_>,
    ) {
        let items = &event.entity.items;
        event.drops.retain(|drop| !items.iter().flatten().any(|s| s == drop));
    }
}

/// The Ender Pouch item.
struct Pouch;

#[mod_api::horizon_part]
impl ItemPart for Pouch {
    fn use_item(&self, host: HostSide, event: &mut UseItemEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        pouch::open(host, event.player, event.stack)
    }

    fn use_on(&self, host: HostSide, event: &mut mod_api::UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        let keys = KEYS.get()?;
        let level = level_here(host)?;
        let pos = event.pos;
        if level.block(host, pos) != Some(keys.chest) {
            return None;
        }
        let sneaking = host
            .player(event.player)
            .and_then(|p| p.is_secondary_use_active(host))
            .unwrap_or(false);
        if !sneaking {
            return None;
        }
        let freq = frequency_at(host, pos)?;
        let place = ItemPlace::PlayerHand { player: event.player, hand: event.hand };
        let _ = host.transaction(|host, tx| {
            if place.edit(host, tx, |stack| pouch::set_frequency(stack, &freq)) {
                Ok(())
            } else {
                Err(())
            }
        });
        Some(InteractionResult::SuccessServer)
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let chest = r.block_key("enderstorage:ender_chest");
        let tank = r.block_key("enderstorage:ender_tank");
        r.block_container(
            &chest,
            ModContainer {
                slots: CHEST_SLOTS,
                faces: std::array::from_fn(|_| (0..CHEST_SLOTS).collect()),
                inputs: (0..CHEST_SLOTS).collect(),
                outputs: (0..CHEST_SLOTS).collect(),
                menu: Some("minecraft:generic_9x3".into()),
            },
        );
        let mut tanks = ModFluidTanks::default();
        tanks.tanks = vec![TANK_CAPACITY];
        tanks.faces = std::array::from_fn(|_| vec![0]);
        tanks.inputs = vec![0];
        tanks.outputs = vec![0];
        r.block_fluids(&tank, tanks);
        let dyes = DYES.map(|d| r.item_key(&format!("minecraft:{d}_dye")));
        let diamond = r.item_key("minecraft:diamond");
        let pouch = r.item_key("enderstorage:ender_pouch");
        let _ = KEYS.set(Keys { chest, tank, dyes, diamond, pouch });
        r.block_part(chest, Storage);
        r.block_part(tank, Storage);
        let kind = r
            .block_entity_type("storage")
            .blocks(&["enderstorage:ender_chest", "enderstorage:ender_tank"])
            .build();
        let _ = r.block_entity_part(&kind, Entity);
        r.item_part(pouch, Pouch);
    }

    fn server_tick(&self, host: &mut dyn ServerHost, _: &mut ServerTickEvent<'_>) {
        lock(&CHESTS).flush(host);
        lock(&TANKS).flush(host);
    }

    fn menu_clicked(&self, host: &mut dyn ServerHost, event: &mut MenuClickedEvent) {
        pouch::clicked(host, event);
    }

    fn menu_closed(&self, _: &mut dyn ServerHost, event: &mut MenuClosedEvent) {
        pouch::closed(event.uuid, event.window);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lid_buttons_count_left_to_right_from_the_front() {
        // Facing south: the viewer stands south looking north, so their left is west (x = 0).
        assert_eq!(spot([0.3, 1.0, 0.5], [0, 1, 0], "south"), Spot::Button(0));
        assert_eq!(spot([0.5, 1.0, 0.5], [0, 1, 0], "south"), Spot::Button(1));
        assert_eq!(spot([0.7, 1.0, 0.5], [0, 1, 0], "south"), Spot::Button(2));
        // Facing north the same point is the other way round.
        assert_eq!(spot([0.3, 1.0, 0.5], [0, 1, 0], "north"), Spot::Button(2));
        // The lid's edge and the front's latch.
        assert_eq!(spot([0.1, 1.0, 0.5], [0, 1, 0], "south"), Spot::Body);
        assert_eq!(spot([0.5, 0.6, 1.0], [0, 0, 1], "south"), Spot::Latch);
        assert_eq!(spot([0.5, 0.6, 0.0], [0, 0, -1], "south"), Spot::Body);
    }

    #[test]
    fn a_dye_recolours_its_button_and_a_diamond_makes_it_personal_once() {
        let freq = Frequency::default();
        assert_eq!(
            edit(&freq, Spot::Button(2), Some(14), false, 7),
            Edit::Colour { button: 2, colour: 14 }
        );
        assert_eq!(edit(&freq, Spot::Button(2), Some(0), false, 7), Edit::None);
        assert_eq!(edit(&freq, Spot::Latch, None, true, 7), Edit::Own);
        let owned = Frequency { owner: Some(7), ..freq };
        assert_eq!(edit(&owned, Spot::Latch, None, true, 9), Edit::None);
        assert!(may_open(&owned, 7) && !may_open(&owned, 9));
    }

    #[test]
    fn personal_and_public_storages_of_one_colour_are_apart() {
        let public = Frequency { colours: [1, 2, 3], owner: None };
        let mine = Frequency { owner: Some(0xabc), ..public };
        let theirs = Frequency { owner: Some(0xdef), ..public };
        assert_ne!(public.key(), mine.key());
        assert_ne!(mine.key(), theirs.key());
    }

    #[test]
    fn a_frequency_round_trips_through_its_saved_form() {
        let freq = Frequency { colours: [15, 0, 9], owner: Some(u128::MAX - 5) };
        let mut nbt = Compound::new();
        freq.save(&mut nbt);
        assert_eq!(Frequency::load(&nbt), freq);
    }
}
