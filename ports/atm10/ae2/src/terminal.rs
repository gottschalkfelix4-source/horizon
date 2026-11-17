//! The ME terminal: a six-row view (`open_menu`) of what the network's cells hold. The game moves
//! nothing in such a view; every click comes here (`menu_clicked`). A click on a listed item takes
//! it out of the network (left: a stack, right: half), a click holding a stack puts it in (right:
//! one), shift-click on a listed item sends a stack to the inventory and on an inventory slot puts
//! the stack into the network. The arrows turn the page; the compass slot sets the search to the
//! item held on the cursor (empty cursor clears it). Only items without components go in (a cell
//! keeps an id and a count).
use std::sync::Mutex;

use mod_api::{
    Compound, Direction, ENERGY, Host, HostTransaction, Identifier, InteractionResult, ItemPlace,
    ItemResource, ItemStack, LevelRef, MenuClickedEvent, MenuRef, MenuReplace, ServerHost, Tag, Text,
};

use crate::cells::*;
use crate::rules::*;
use crate::view::{self, Slot};
use crate::{cells_of, id_at, network_extract, network_insert, offset};

struct View {
    player: u128,
    window: i32,
    pos: [i32; 3],
    page: usize,
    search: Option<String>,
}

static VIEWS: Mutex<Vec<View>> = Mutex::new(Vec::new());

fn views() -> std::sync::MutexGuard<'static, Vec<View>> {
    VIEWS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Every block of the network `pos` belongs to.
fn nodes_of(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Vec<[i32; 3]> {
    flood(pos, |p| {
        let Some(here) = id_at(host, level, p) else { return Vec::new() };
        Direction::ALL
            .into_iter()
            .map(|d| offset(p, d))
            .filter(|n| id_at(host, level, *n).is_some_and(|o| connects(&here, &o)))
            .collect()
    })
}

struct Net {
    drives: Vec<[i32; 3]>,
    powered: bool,
}

fn net(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3]) -> Net {
    let nodes = nodes_of(&*host, level, pos);
    let (mut drives, mut cells) = (Vec::new(), Vec::new());
    for &n in &nodes {
        match id_at(&*host, level, n).as_deref().and_then(parse_block) {
            Some(Kind::Device(Device::Drive)) => drives.push(n),
            Some(Kind::Device(d)) if d.is_cell() => cells.push(n),
            _ => {}
        }
    }
    let mut stored = 0u64;
    for &c in &cells {
        if let Some(cap) = level.capability(host, &ENERGY, c, Some(Direction::Up)) {
            stored += cap.amount(host) as u64;
        }
    }
    Net { drives, powered: stored > 0 }
}

/// The network's items, as the view lists them.
fn items_of(host: &dyn Host, level: LevelRef, drives: &[[i32; 3]]) -> Vec<(String, u64)> {
    let mut all: Vec<(String, u64)> = Vec::new();
    for (_, _, _, stack) in cells_of(host, level, drives) {
        for (id, n) in Store::of_stack(&stack).items {
            match all.iter_mut().find(|(i, _)| *i == id) {
                Some((_, c)) => *c += n,
                None => all.push((id, n)),
            }
        }
    }
    view::listing(all)
}

fn named(mut stack: ItemStack, name: &str) -> ItemStack {
    let mut patch = Compound::new();
    patch.insert("minecraft:custom_name".into(), Tag::String(name.into()));
    let _ = stack.apply_components(&patch);
    stack
}

fn stack_of(host: &dyn Host, id: &str, count: u64) -> Option<ItemStack> {
    let key = host.item_key(&Identifier::parse(id).ok()?)?;
    let max = host.max_stack_size(key).unwrap_or(64) as u64;
    ItemStack::new(key, count.min(max).max(1) as u32)
}

/// The 54 slots of the view for `page` and `search`.
fn render(host: &dyn Host, level: LevelRef, v: &View, drives: &[[i32; 3]]) -> (usize, Vec<Option<ItemStack>>) {
    let all = items_of(host, level, drives);
    let shown = view::filter(&all, v.search.as_deref());
    let (page, here) = view::page_of(&shown, v.page);
    let mut slots: Vec<Option<ItemStack>> = vec![None; 54];
    for (i, (id, n)) in here.iter().enumerate() {
        slots[i] = stack_of(host, id, *n);
    }
    let pages = view::pages(shown.len());
    let control = |id: &str, name: String| stack_of(host, id, 1).map(|s| named(s, &name));
    slots[view::PREV] = control("minecraft:arrow", format!("Page {} of {}: previous", page + 1, pages));
    slots[view::NEXT] = control("minecraft:spectral_arrow", format!("Page {} of {}: next", page + 1, pages));
    let search = v.search.as_deref().map_or("Search: all (click with an item to filter)".to_string(), |s| format!("Search: {s}"));
    slots[view::SEARCH] = control("minecraft:compass", search);
    (page, slots)
}

pub fn open(host: &mut dyn ServerHost, player: u128, pos: [i32; 3]) -> Option<InteractionResult> {
    let level = host.level(host.dimension())?;
    let n = net(host, level, pos);
    if !n.powered {
        if let Some(p) = host.player(player) {
            let _ = p.action_bar(host, Text::literal("The network has no power"));
        }
        return Some(InteractionResult::Fail);
    }
    let mut v = View { player, window: 0, pos, page: 0, search: None };
    let (_, items) = render(&*host, level, &v, &n.drives);
    let title = Text::translate("block.ae2.terminal", Vec::new());
    v.window = host.open_menu(player, view::ROWS, title, items, MenuReplace::CloseFirst).ok().flatten()?;
    let mut all = views();
    all.retain(|x| x.player != player);
    all.push(v);
    Some(InteractionResult::SuccessServer)
}

pub fn closed(player: u128, window: i32) {
    views().retain(|v| !(v.player == player && v.window == window));
}

/// Replaces what `place` holds with `stack`, all or nothing.
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

fn max_of(host: &dyn Host, stack: &ItemStack) -> u32 {
    host.max_stack_size(stack.item()).unwrap_or(64)
}

fn plain(stack: &ItemStack) -> bool {
    stack.components().is_none_or(|c| c.as_nbt().is_empty())
}

/// Puts `stack` into the player's inventory slots of the view (hotbar last): what did not fit.
fn give(host: &mut dyn ServerHost, menu: MenuRef, stack: ItemStack) -> Option<ItemStack> {
    let count = menu.slot_count(host).unwrap_or(54 + 36);
    let max = max_of(&*host, &stack);
    let mut left = stack.count();
    for pass in 0..2 {
        for index in (54..count).rev() {
            if left == 0 {
                return None;
            }
            let held = menu.slot(host, index).flatten();
            let room = match (&held, pass) {
                (Some(h), 0) if h.same_item(&stack) && h.is_stackable_with(&stack) => max.saturating_sub(h.count()),
                (None, 1) => max,
                _ => 0,
            };
            let put_n = left.min(room);
            if put_n == 0 {
                continue;
            }
            let mut after = stack.clone();
            let _ = after.set_count(held.as_ref().map_or(0, |h| h.count()) + put_n);
            if put(host, &ItemPlace::MenuSlot { menu, slot: index }, Some(after)) {
                left -= put_n;
            }
        }
    }
    (left > 0).then(|| {
        let mut s = stack;
        let _ = s.set_count(left);
        s
    })
}

pub fn clicked(host: &mut dyn ServerHost, event: &MenuClickedEvent) {
    let (pos, search_now, page_now) = {
        let all = views();
        match all.iter().find(|v| v.player == event.uuid && v.window == event.window) {
            Some(v) => (v.pos, v.search.clone(), v.page),
            None => return,
        }
    };
    let (Some(level), Some(menu)) = (host.level(host.dimension()), host.menu(event.uuid, event.window)) else { return };
    let n = net(host, level, pos);
    let mut page = page_now;
    let mut search = search_now.clone();
    if event.slot >= 0 && n.powered {
        let cursor = menu.carried(host).flatten();
        let cursor_place = ItemPlace::PlayerCursor { menu };
        match (view::slot_kind(event.slot as usize), event.mode) {
            (Slot::Prev, 0) => page = page.saturating_sub(1),
            (Slot::Next, 0) => page += 1,
            (Slot::Search, 0) => {
                search = cursor
                    .as_ref()
                    .map(|c| c.item().id(&*host))
                    .map(|id| id.split_once(':').map_or(id.clone(), |(_, p)| p.to_string()));
                page = 0;
            }
            (Slot::Item(i), mode @ (0 | 1)) => {
                let (shown, _) = {
                    let all = items_of(&*host, level, &n.drives);
                    let shown = view::filter(&all, search.as_deref());
                    let (p, here) = view::page_of(&shown, page);
                    (here.get(i).cloned(), p)
                };
                match (cursor, shown) {
                    // a stack held: it goes into the network
                    (Some(c), _) if mode == 0 && plain(&c) => {
                        let want = if event.button == 1 { 1 } else { c.count() };
                        let id = c.item().id(&*host);
                        let took = network_insert(host, level, &n.drives, &id, want as u64) as u32;
                        if took > 0 {
                            // `shrink` leaves None when nothing is left (a count of 0 is not a stack)
                            let _ = put(host, &cursor_place, c.clone().shrink(took));
                        }
                    }
                    // nothing held: a stack (or half) comes out
                    (None, Some((id, count))) => {
                        if let Some(proto) = stack_of(&*host, &id, 1) {
                            let max = max_of(&*host, &proto) as u64;
                            let mut want = count.min(max);
                            if mode == 0 && event.button == 1 {
                                want = want.div_ceil(2);
                            }
                            let got = network_extract(host, level, &n.drives, &id, want);
                            if got > 0 {
                                let mut out = proto;
                                let _ = out.set_count(got as u32);
                                if mode == 1 {
                                    if let Some(left) = give(host, menu, out) {
                                        let back = network_insert(host, level, &n.drives, &id, left.count() as u64);
                                        let _ = back;
                                    }
                                } else {
                                    let _ = put(host, &cursor_place, Some(out));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            // the player's own slots: shift-click sends the stack into the network
            (Slot::Player(i), 1) => {
                if let Some(stack) = menu.slot(host, i).flatten().filter(plain) {
                    let id = stack.item().id(&*host);
                    let took = network_insert(host, level, &n.drives, &id, stack.count() as u64) as u32;
                    if took > 0 {
                        let _ = put(host, &ItemPlace::MenuSlot { menu, slot: i }, stack.clone().shrink(took));
                    }
                }
            }
            // the player's own slots: an ordinary pick-up and place with the cursor
            (Slot::Player(i), 0) => {
                let held = menu.slot(host, i).flatten();
                let place = ItemPlace::MenuSlot { menu, slot: i };
                let (cursor_after, slot_after) = pickup(cursor, held, event.button == 1, &*host);
                if put(host, &place, slot_after) {
                    let _ = put(host, &cursor_place, cursor_after);
                }
            }
            _ => {}
        }
    }
    let mut all = views();
    let Some(v) = all.iter_mut().find(|v| v.player == event.uuid && v.window == event.window) else { return };
    v.search = search;
    v.page = page;
    let (shown_page, items) = render(&*host, level, v, &n.drives);
    v.page = shown_page;
    drop(all);
    let _ = host.menu_set_items(event.uuid, event.window, items);
}

/// A left (or, `right`, a right) click with `cursor` on a slot holding `slot`: what each holds after.
fn pickup(
    cursor: Option<ItemStack>,
    slot: Option<ItemStack>,
    right: bool,
    host: &dyn ServerHost,
) -> (Option<ItemStack>, Option<ItemStack>) {
    let with = |s: &ItemStack, n: u32| {
        let mut c = s.clone();
        let _ = c.set_count(n.max(1));
        c
    };
    match (cursor, slot) {
        (None, None) => (None, None),
        (None, Some(s)) => {
            if right {
                let take = s.count().div_ceil(2);
                let left = s.count() - take;
                (Some(with(&s, take)), (left > 0).then(|| with(&s, left)))
            } else {
                (Some(s), None)
            }
        }
        (Some(c), None) => {
            let max = max_of(host, &c);
            let put = if right { 1 } else { c.count().min(max) };
            let rest = c.count() - put;
            ((rest > 0).then(|| with(&c, rest)), Some(with(&c, put)))
        }
        (Some(c), Some(s)) if c.is_stackable_with(&s) => {
            let max = max_of(host, &c);
            let put = (if right { 1 } else { c.count() }).min(max.saturating_sub(s.count()));
            let rest = c.count() - put;
            ((rest > 0).then(|| with(&c, rest)), Some(with(&s, s.count() + put)))
        }
        (Some(c), Some(s)) => (Some(s), Some(c)),
    }
}
