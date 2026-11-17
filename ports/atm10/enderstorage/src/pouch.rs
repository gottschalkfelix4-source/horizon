//! The Ender Pouch: the chest storage of the frequency it carries, opened from anywhere.
//!
//! The pouch opens a three-row chest view (`open_menu`) showing its frequency's shared slots.
//! The game moves nothing in such a view: each click comes here (`menu_clicked`), which carries
//! out a chest's pick-up, place, swap and shift-click on the shared storage, the cursor and the
//! player's inventory, then shows the view again. Ender Chests of that frequency pick the change
//! up on their next tick, as they do any other.
use std::sync::Mutex;

use mod_api::{
    Compound, HostTransaction, InteractionResult, ItemPlace, ItemResource, ItemStack, MenuClickedEvent, MenuRef,
    MenuReplace, ServerHost, Tag, Text,
};

use crate::{CHESTS, CHEST_SLOTS, Frequency, lock, may_open, tell};

/// A stack as the click rules see it, so they can be tested without a game.
pub trait Pile: Clone {
    fn count(&self) -> u32;
    fn with_count(&self, count: u32) -> Self;
    /// Whether the two stack together (same item and components).
    fn stacks_with(&self, other: &Self) -> bool;
}

impl Pile for ItemStack {
    fn count(&self) -> u32 {
        ItemStack::count(self)
    }
    fn with_count(&self, count: u32) -> Self {
        let mut s = self.clone();
        let _ = s.set_count(count.max(1));
        s
    }
    fn stacks_with(&self, other: &Self) -> bool {
        self.is_stackable_with(other)
    }
}

/// A left (`right` false) or right click with `cursor` on a slot holding `slot`, whose stacks
/// hold at most `max`: what the cursor and the slot hold afterwards (`ClickType.PICKUP`).
pub fn pickup<P: Pile>(
    cursor: Option<P>,
    slot: Option<P>,
    right: bool,
    max: u32,
) -> (Option<P>, Option<P>) {
    match (cursor, slot) {
        (None, None) => (None, None),
        (None, Some(s)) => {
            if right {
                let take = s.count().div_ceil(2);
                let left = s.count() - take;
                (Some(s.with_count(take)), (left > 0).then(|| s.with_count(left)))
            } else {
                (Some(s), None)
            }
        }
        (Some(c), None) => {
            let put = if right { 1 } else { c.count().min(max) };
            let rest = c.count() - put;
            ((rest > 0).then(|| c.with_count(rest)), Some(c.with_count(put)))
        }
        (Some(c), Some(s)) if c.stacks_with(&s) => {
            let want = if right { 1 } else { c.count() };
            let put = want.min(max.saturating_sub(s.count()));
            let rest = c.count() - put;
            (
                (rest > 0).then(|| c.with_count(rest)),
                Some(s.with_count(s.count() + put)),
            )
        }
        (Some(c), Some(s)) => (Some(s), Some(c)),
    }
}

/// Put `stack` into `slots`: first onto stacks it joins, then into empty slots. What did not fit.
pub fn insert<P: Pile>(slots: &mut [Option<P>], stack: P, max: u32) -> Option<P> {
    let mut left = stack.count();
    for cell in slots.iter_mut() {
        if left == 0 {
            break;
        }
        if let Some(s) = cell.as_mut().filter(|s| s.stacks_with(&stack) && s.count() < max) {
            let put = left.min(max - s.count());
            *s = s.with_count(s.count() + put);
            left -= put;
        }
    }
    for cell in slots.iter_mut() {
        if left == 0 {
            break;
        }
        if cell.is_none() {
            let put = left.min(max);
            *cell = Some(stack.with_count(put));
            left -= put;
        }
    }
    (left > 0).then(|| stack.with_count(left))
}

/// One open pouch view: who, which window, which storage.
struct View {
    player: u128,
    window: i32,
    key: String,
}

static VIEWS: Mutex<Vec<View>> = Mutex::new(Vec::new());

const DATA_KEY: &str = "enderstorage_frequency";

/// The frequency a pouch carries (white, white, white and public until it took one).
pub fn frequency_of(stack: &ItemStack) -> Frequency {
    let tag = stack
        .components()
        .and_then(|c| c.as_nbt().get(&"minecraft:custom_data".into()))
        .and_then(|data| data.get(DATA_KEY));
    match tag {
        Some(Tag::Compound(f)) => {
            let mut nbt = Compound::new();
            nbt.insert("frequency".into(), Tag::Compound(f.clone()));
            Frequency::load(&nbt)
        }
        _ => Frequency::default(),
    }
}

/// Make `stack` carry `freq`, keeping its other custom data.
pub fn set_frequency(stack: &mut ItemStack, freq: &Frequency) {
    let mut data = match stack
        .components()
        .and_then(|c| c.as_nbt().get(&"minecraft:custom_data".into()))
    {
        Some(Tag::Compound(data)) => data.clone(),
        _ => Compound::new(),
    };
    data.insert(DATA_KEY.into(), freq.tag());
    let mut patch = Compound::new();
    patch.insert("minecraft:custom_data".into(), Tag::Compound(data));
    let _ = stack.apply_components(&patch);
}

fn shared_items(host: &dyn ServerHost, key: &str) -> Vec<Option<ItemStack>> {
    let mut chests = lock(&CHESTS);
    chests.open(host);
    chests.get(key, || vec![None; CHEST_SLOTS]).value.clone()
}

/// Open the pouch's storage for `player`.
pub fn open(host: &mut dyn ServerHost, player: u128, stack: &ItemStack) -> Option<InteractionResult> {
    let freq = frequency_of(stack);
    if !may_open(&freq, player) {
        tell(host, player, "enderstorage.not_owner");
        return Some(InteractionResult::Fail);
    }
    let key = freq.key();
    let items = shared_items(host, &key);
    let title = Text::translate("item.enderstorage.ender_pouch", Vec::new());
    let window = host
        .open_menu(player, 3, title, items, MenuReplace::CloseFirst)
        .ok()
        .flatten()?;
    let mut views = lock(&VIEWS);
    views.retain(|v| v.player != player);
    views.push(View { player, window, key });
    Some(InteractionResult::SuccessServer)
}

pub fn closed(player: u128, window: i32) {
    lock(&VIEWS).retain(|v| !(v.player == player && v.window == window));
}

/// Replace what `place` holds with `stack`, all or nothing.
fn put(host: &mut dyn ServerHost, place: &ItemPlace, stack: Option<ItemStack>) -> bool {
    host.transaction(|host, tx| {
        if let Some(old) = place.stack(host) {
            let n = old.count();
            if place.extract(host, tx, &ItemResource::of(&old), n) != n {
                return Err(());
            }
        }
        if let Some(new) = stack {
            let n = new.count();
            if place.insert(host, tx, &ItemResource::of(&new), n) != n {
                return Err(());
            }
        }
        Ok(())
    })
    .is_ok()
}

fn max_of(host: &dyn ServerHost, stack: Option<&ItemStack>) -> u32 {
    stack
        .and_then(|s| host.max_stack_size(s.item()))
        .unwrap_or(64)
}

/// Carry out a click in a pouch view.
pub fn clicked(host: &mut dyn ServerHost, event: &MenuClickedEvent) {
    let key = {
        let views = lock(&VIEWS);
        match views.iter().find(|v| v.player == event.uuid && v.window == event.window) {
            Some(v) => v.key.clone(),
            None => return,
        }
    };
    let Some(menu) = host.menu(event.uuid, event.window) else {
        return;
    };
    let mut items = shared_items(host, &key);
    let before = items.clone();
    let slot = event.slot;
    match event.mode {
        // PICKUP
        0 if slot >= 0 => {
            let cursor = menu.carried(host).flatten();
            let index = slot as usize;
            let held = if index < CHEST_SLOTS {
                items[index].clone()
            } else {
                menu.slot(host, index).flatten()
            };
            let max = max_of(host, cursor.as_ref().or(held.as_ref()));
            let (cursor_after, slot_after) = pickup(cursor, held, event.button == 1, max);
            let cursor_place = ItemPlace::PlayerCursor { menu };
            if index < CHEST_SLOTS {
                if put(host, &cursor_place, cursor_after) {
                    items[index] = slot_after;
                }
            } else {
                let place = ItemPlace::MenuSlot { menu, slot: index };
                if put(host, &place, slot_after) {
                    let _ = put(host, &cursor_place, cursor_after);
                }
            }
        }
        // QUICK_MOVE
        1 if slot >= 0 => {
            let index = slot as usize;
            if index < CHEST_SLOTS {
                if let Some(stack) = items[index].take() {
                    let left = give(host, menu, stack);
                    items[index] = left;
                }
            } else if let Some(stack) = menu.slot(host, index).flatten() {
                let max = max_of(host, Some(&stack));
                let left = insert(&mut items, stack, max);
                let place = ItemPlace::MenuSlot { menu, slot: index };
                if !put(host, &place, left) {
                    items = before.clone();
                }
            }
        }
        _ => {}
    }
    if items != before {
        let mut chests = lock(&CHESTS);
        let shared = chests.get(&key, || vec![None; CHEST_SLOTS]);
        shared.value = items.clone();
        shared.rev += 1;
        chests.mark_dirty();
    }
    // Show the result (or undo a client's prediction of a click that did nothing).
    let _ = host.menu_set_items(event.uuid, event.window, items);
}

/// Move `stack` into the player's inventory through the view's own inventory slots, hotbar
/// last as a chest's shift-click does: what did not fit.
fn give(host: &mut dyn ServerHost, menu: MenuRef, stack: ItemStack) -> Option<ItemStack> {
    let count = menu.slot_count(host).unwrap_or(CHEST_SLOTS + 36);
    let mut left = Some(stack);
    let order: Vec<usize> = (CHEST_SLOTS..count).rev().collect();
    for pass in 0..2 {
        for &index in &order {
            let Some(stack) = left.clone() else { return None };
            let held = menu.slot(host, index).flatten();
            let fits = match (&held, pass) {
                (Some(h), 0) => h.is_stackable_with(&stack),
                (None, 1) => true,
                _ => false,
            };
            if !fits {
                continue;
            }
            let max = max_of(host, Some(&stack));
            let mut cells = [held];
            let rest = insert(&mut cells, stack, max);
            let [after] = cells;
            if put(host, &ItemPlace::MenuSlot { menu, slot: index }, after) {
                left = rest;
            }
        }
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct S(u8, u32);
    impl Pile for S {
        fn count(&self) -> u32 {
            self.1
        }
        fn with_count(&self, count: u32) -> Self {
            S(self.0, count)
        }
        fn stacks_with(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }

    #[test]
    fn left_and_right_clicks_pick_up_place_and_swap_as_a_chest_does() {
        assert_eq!(pickup(None, Some(S(1, 9)), true, 64), (Some(S(1, 5)), Some(S(1, 4))));
        assert_eq!(pickup(Some(S(1, 5)), None, true, 64), (Some(S(1, 4)), Some(S(1, 1))));
        assert_eq!(pickup(Some(S(1, 50)), Some(S(1, 40)), false, 64), (Some(S(1, 26)), Some(S(1, 64))));
        assert_eq!(pickup(Some(S(2, 3)), Some(S(1, 40)), false, 64), (Some(S(1, 40)), Some(S(2, 3))));
    }

    #[test]
    fn a_shift_click_fills_matching_stacks_before_empty_slots() {
        let mut slots = [None, Some(S(1, 60)), Some(S(2, 1)), None];
        let left = insert(&mut slots, S(1, 70), 64);
        assert_eq!(left, None);
        assert_eq!(slots, [Some(S(1, 64)), Some(S(1, 64)), Some(S(2, 1)), Some(S(1, 2))]);
        let mut full = [Some(S(3, 64))];
        assert_eq!(insert(&mut full, S(3, 5), 64), Some(S(3, 5)));
    }
}
