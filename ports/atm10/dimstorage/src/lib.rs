//! DimStorage for Horizon: Dimensional Chests and Tanks whose contents are shared by every block
//! of the same numbered frequency, in every dimension, plus the Dimensional Tablet that opens a
//! chest frequency from anywhere.
//!
//! Written from DimStorage's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and what a check against the NeoForge
//! original still has to confirm, is in README.md.
//!
//! A frequency is a number and, once a player locks it, that player: a locked frequency opens
//! only for its owner and is a different storage from the public one of the same number. The
//! contents live in the world's shared store (`store.rs`); each block entity holds a copy that
//! hoppers and the vanilla chest menu use, kept in step every tick.
//!
//! The original sets a frequency in a screen. Horizon's mod API has no screen with a number
//! field yet, so here an ender pearl in hand steps the number by one and an eye of ender by ten
//! (down while sneaking), and using the block with an empty hand while sneaking toggles the lock.
use std::sync::{Mutex, OnceLock};

use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockKey, BlockPart, Compound, Hooks, Host, HostSide, HostTransaction,
    InteractionResult, ItemKey, ItemPart, ItemPlace, LevelRef,
    MenuClickedEvent, MenuClosedEvent, ModContainer, ModFluidTanks, PreRemoveSideEffectsEvent,
    Registrar, RegistrationContext, ServerHost, ServerTickEvent, Tag, Text, UseItemEvent,
    UseItemOnEvent, UseWithoutItemEvent,
};

pub mod store;
pub mod tablet;

use store::{Items, Store, Sync, Tank};

/// Slots of a Dimensional Chest: six rows.
pub const CHEST_SLOTS: usize = 54;
/// A Dimensional Tank's capacity in millibuckets (16 buckets).
pub const TANK_CAPACITY: i32 = 16_000;
/// The highest frequency number.
pub const MAX_FREQUENCY: i32 = 9_999;

/// A frequency: a number, and the owner a locked storage belongs to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Frequency {
    pub id: i32,
    pub owner: Option<u128>,
    pub locked: bool,
}

impl Frequency {
    /// The shared store's key: chests and tanks, and locked and public storages, are apart.
    pub fn key(&self) -> String {
        match (self.locked, self.owner) {
            (true, Some(owner)) => format!("{owner:032x}/{}", self.id),
            _ => format!("public/{}", self.id),
        }
    }

    pub fn load(nbt: &Compound) -> Self {
        let mut freq = Frequency::default();
        if let Some(Tag::Compound(f)) = nbt.get(&"frequency".into()) {
            if let Some(Tag::Int(id)) = f.get(&"id".into()) {
                freq.id = (*id).clamp(0, MAX_FREQUENCY);
            }
            if let Some(Tag::Byte(l)) = f.get(&"locked".into()) {
                freq.locked = *l != 0;
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
        f.insert("id".into(), Tag::Int(self.id));
        f.insert("locked".into(), Tag::Byte(i8::from(self.locked)));
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

/// Whether `player` may open a storage of `freq`.
pub fn may_open(freq: &Frequency, player: u128) -> bool {
    !freq.locked || freq.owner.is_none_or(|owner| owner == player)
}

/// The number moved by `delta`, kept in 0..=[`MAX_FREQUENCY`]; a locked frequency only moves for
/// its owner.
pub fn step(freq: &Frequency, delta: i32, player: u128) -> Option<Frequency> {
    if !may_open(freq, player) {
        return None;
    }
    Some(Frequency { id: (freq.id + delta).clamp(0, MAX_FREQUENCY), ..*freq })
}

/// The lock toggled by `player`: an unlocked frequency becomes theirs and locked, a locked one
/// unlocks for its owner only.
pub fn toggle_lock(freq: &Frequency, player: u128) -> Option<Frequency> {
    if !may_open(freq, player) {
        return None;
    }
    Some(if freq.locked {
        Frequency { locked: false, ..*freq }
    } else {
        Frequency { locked: true, owner: Some(player), ..*freq }
    })
}

/// Keys known once registration ran.
struct Keys {
    chest: BlockKey,
    tank: BlockKey,
    pearl: ItemKey,
    eye: ItemKey,
    tablet: ItemKey,
}

static KEYS: OnceLock<Keys> = OnceLock::new();

/// The shared chest inventories and tanks of the open world.
pub static CHESTS: Mutex<Store<Items>> = Mutex::new(Store::new("chests.nbt"));
pub static TANKS: Mutex<Store<Tank>> = Mutex::new(Store::new("tanks.nbt"));

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn is_chest(id: &str) -> bool {
    id == "dimstorage:dimensional_chest"
}

fn is_tank(id: &str) -> bool {
    id == "dimstorage:dimensional_tank"
}

fn level_here(host: &dyn ServerHost) -> Option<LevelRef> {
    host.level(host.dimension())
}

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

fn tell_frequency(host: &mut dyn ServerHost, player: u128, freq: &Frequency) {
    if let Some(p) = host.player(player) {
        let _ = p.action_bar(
            host,
            Text::translate("dimstorage.frequency", vec![Text::literal(&freq.id.to_string())]),
        );
    }
}

fn sneaking(host: &dyn ServerHost, player: u128) -> bool {
    host.player(player)
        .and_then(|p| p.is_secondary_use_active(host))
        .unwrap_or(false)
}

/// The two blocks: frequency clicks, opening, and the owner lock.
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
        let item = stack.item();
        if item == keys.tablet {
            // The tablet takes the frequency; `Tablet::use_on` handles it.
            return None;
        }
        let freq = frequency_at(host, pos)?;
        let delta = if item == keys.pearl {
            1
        } else if item == keys.eye {
            10
        } else {
            return use_tank_container(host, event, keys);
        };
        let delta = if sneaking(host, click.player) { -delta } else { delta };
        match step(&freq, delta, click.player) {
            Some(new) => {
                if set_frequency_at(host, pos, new) {
                    tell_frequency(host, click.player, &new);
                }
                Some(InteractionResult::SuccessServer)
            }
            None => {
                tell(host, click.player, "dimstorage.locked");
                Some(InteractionResult::Fail)
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
        let block = level.block(host, click.pos)?;
        let freq = frequency_at(host, click.pos)?;
        if sneaking(host, click.player) {
            return match toggle_lock(&freq, click.player) {
                Some(new) => {
                    let key = if new.locked { "dimstorage.locked_on" } else { "dimstorage.locked_off" };
                    if set_frequency_at(host, click.pos, new) {
                        tell(host, click.player, key);
                    }
                    Some(InteractionResult::SuccessServer)
                }
                None => {
                    tell(host, click.player, "dimstorage.locked");
                    Some(InteractionResult::Fail)
                }
            };
        }
        if block != keys.chest {
            return None;
        }
        if !may_open(&freq, click.player) {
            tell(host, click.player, "dimstorage.locked");
            return Some(InteractionResult::Fail);
        }
        let entity = host.block_entity(level, click.pos)?;
        let player = host.player(click.player)?;
        let _ = player.open_menu(host, entity);
        Some(InteractionResult::Consume)
    }
}

/// A bucket or other fluid container on a Dimensional Tank: fill or empty it, as on any tank.
fn use_tank_container(
    host: &mut dyn ServerHost,
    event: &UseItemOnEvent<'_>,
    keys: &Keys,
) -> Option<InteractionResult> {
    let click = event.click;
    let level = level_here(host)?;
    if level.block(host, click.pos) != Some(keys.tank) {
        return None;
    }
    let freq = frequency_at(host, click.pos)?;
    if !may_open(&freq, click.player) {
        tell(host, click.player, "dimstorage.locked");
        return Some(InteractionResult::Fail);
    }
    let tank = level.capability(host, &mod_api::FLUIDS, click.pos, None)?;
    let player = host.player(click.player)?;
    let moved =
        mod_api::fluid_util::interact_with_handler(host, player, click.hand, Some(click.pos), &tank);
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

/// The Dimensional Tablet item.
struct Tablet;

#[mod_api::horizon_part]
impl ItemPart for Tablet {
    fn use_item(&self, host: HostSide, event: &mut UseItemEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        tablet::open(host, event.player, event.stack)
    }

    /// Sneak-using the tablet on a chest binds it to the chest's frequency.
    fn use_on(&self, host: HostSide, event: &mut mod_api::UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        let keys = KEYS.get()?;
        let level = level_here(host)?;
        let pos = event.pos;
        if level.block(host, pos) != Some(keys.chest) || !sneaking(host, event.player) {
            return None;
        }
        let freq = frequency_at(host, pos)?;
        if !may_open(&freq, event.player) {
            tell(host, event.player, "dimstorage.locked");
            return Some(InteractionResult::Fail);
        }
        let place = ItemPlace::PlayerHand { player: event.player, hand: event.hand };
        let _ = host.transaction(|host, tx| {
            if place.edit(host, tx, |stack| tablet::set_frequency(stack, &freq)) {
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
        let chest = r.block_key("dimstorage:dimensional_chest");
        let tank = r.block_key("dimstorage:dimensional_tank");
        r.block_container(
            &chest,
            ModContainer {
                slots: CHEST_SLOTS,
                faces: std::array::from_fn(|_| (0..CHEST_SLOTS).collect()),
                inputs: (0..CHEST_SLOTS).collect(),
                outputs: (0..CHEST_SLOTS).collect(),
                menu: Some("minecraft:generic_9x6".into()),
            },
        );
        let mut tanks = ModFluidTanks::default();
        tanks.tanks = vec![TANK_CAPACITY];
        tanks.faces = std::array::from_fn(|_| vec![0]);
        tanks.inputs = vec![0];
        tanks.outputs = vec![0];
        r.block_fluids(&tank, tanks);
        let pearl = r.item_key("minecraft:ender_pearl");
        let eye = r.item_key("minecraft:ender_eye");
        let tablet_key = r.item_key("dimstorage:dimensional_tablet");
        let _ = KEYS.set(Keys { chest, tank, pearl, eye, tablet: tablet_key });
        r.block_part(chest, Storage);
        r.block_part(tank, Storage);
        let kind = r
            .block_entity_type("storage")
            .blocks(&["dimstorage:dimensional_chest", "dimstorage:dimensional_tank"])
            .build();
        let _ = r.block_entity_part(&kind, Entity);
        r.item_part(tablet_key, Tablet);
    }

    fn server_tick(&self, host: &mut dyn ServerHost, _: &mut ServerTickEvent<'_>) {
        lock(&CHESTS).flush(host);
        lock(&TANKS).flush(host);
    }

    fn menu_clicked(&self, host: &mut dyn ServerHost, event: &mut MenuClickedEvent) {
        tablet::clicked(host, event);
    }

    fn menu_closed(&self, _: &mut dyn ServerHost, event: &mut MenuClosedEvent) {
        tablet::closed(event.uuid, event.window);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_number_steps_within_bounds() {
        let f = Frequency { id: 5, ..Default::default() };
        assert_eq!(step(&f, 10, 1).unwrap().id, 15);
        assert_eq!(step(&f, -10, 1).unwrap().id, 0);
        let top = Frequency { id: MAX_FREQUENCY - 1, ..Default::default() };
        assert_eq!(step(&top, 10, 1).unwrap().id, MAX_FREQUENCY);
    }

    #[test]
    fn locking_gives_the_storage_to_the_player_and_only_they_unlock_or_retune_it() {
        let open = Frequency { id: 3, ..Default::default() };
        let mine = toggle_lock(&open, 7).unwrap();
        assert!(mine.locked && mine.owner == Some(7));
        assert!(toggle_lock(&mine, 9).is_none());
        assert!(step(&mine, 1, 9).is_none());
        assert!(!may_open(&mine, 9) && may_open(&mine, 7));
        let back = toggle_lock(&mine, 7).unwrap();
        assert!(!back.locked && may_open(&back, 9));
    }

    #[test]
    fn locked_and_public_storages_of_one_number_are_apart() {
        let public = Frequency { id: 12, ..Default::default() };
        let mine = Frequency { id: 12, owner: Some(0xabc), locked: true };
        let theirs = Frequency { id: 12, owner: Some(0xdef), locked: true };
        assert_ne!(public.key(), mine.key());
        assert_ne!(mine.key(), theirs.key());
        // A kept owner without the lock is still the public storage.
        assert_eq!(public.key(), Frequency { locked: false, ..mine }.key());
    }

    #[test]
    fn a_frequency_round_trips_through_its_saved_form() {
        let freq = Frequency { id: 4321, owner: Some(u128::MAX - 5), locked: true };
        let mut nbt = Compound::new();
        freq.save(&mut nbt);
        assert_eq!(Frequency::load(&nbt), freq);
    }
}
