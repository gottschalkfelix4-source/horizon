//! Construction Sticks for Horizon: right-click a block's face with a stick and the connected
//! surface of that block grows one layer outwards, from the blocks in your inventory.
//!
//! Written from Construction Sticks' public description and observed behaviour, never from its
//! code (docs/modpacks.md, "Licences"). What a harness against the NeoForge original still has
//! to confirm is in README.md.
//!
//! How it works: each stick item carries an `ItemPart` whose `use_on` plans the placements on
//! the server: starting at the clicked block, it walks the clicked face's plane through
//! connected blocks of the same type whose neighbour in front of the face is free, up to the
//! stick's limit and the blocks the player has. Each target is placed as the player
//! (`PlayerRef::place_block`), then the blocks are taken from the inventory and the stick wears
//! by one durability point per block.
use std::collections::{HashSet, VecDeque};

use mod_api::{
    BlockKey, BlockState, EquipmentSlot, Hand, Host, HostBlockStates, HostSide, InteractionResult,
    ItemKey, ItemPart, ItemStack, LevelRef, PlayerRef, Registrar, RegistrationContext, ServerHost,
    SoundSource, UseOnEvent,
};

/// The sticks: item id and how many blocks one click places at most.
pub const STICKS: [(&str, usize); 5] = [
    ("constructionstick:wooden_stick", 9),
    ("constructionstick:stone_stick", 27),
    ("constructionstick:iron_stick", 81),
    ("constructionstick:diamond_stick", 243),
    ("constructionstick:netherite_stick", 729),
];

/// The four directions in the plane perpendicular to `face`.
fn plane_steps(face: [i32; 3]) -> [[i32; 3]; 4] {
    if face[0] != 0 {
        [[0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]]
    } else if face[1] != 0 {
        [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]]
    } else {
        [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0]]
    }
}

fn add(a: [i32; 3], b: [i32; 3]) -> [i32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// Where a click on `face` of the block at `start` places blocks: breadth first from `start`
/// through the face's plane, over blocks that `matches` and whose neighbour in front of the
/// face is `free`, nearest first, at most `limit`. Returns the positions to fill (each block's
/// neighbour in front of the face).
pub fn plan(
    start: [i32; 3],
    face: [i32; 3],
    limit: usize,
    mut matches: impl FnMut([i32; 3]) -> bool,
    mut free: impl FnMut([i32; 3]) -> bool,
) -> Vec<[i32; 3]> {
    let steps = plane_steps(face);
    let mut seen = HashSet::from([start]);
    let mut queue = VecDeque::from([start]);
    let mut out = Vec::new();
    while let Some(pos) = queue.pop_front() {
        if out.len() >= limit {
            break;
        }
        let target = add(pos, face);
        if !matches(pos) || !free(target) {
            continue;
        }
        out.push(target);
        for step in steps {
            let next = add(pos, step);
            if seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    out
}

/// Which slots to take `need` items from, given the slots holding the item as `(slot, count)`:
/// lowest slot first, each entry the slot and the count it keeps.
pub fn take_from(slots: &[(u32, u32)], need: u32) -> Vec<(u32, u32)> {
    let mut left = need;
    let mut out = Vec::new();
    for &(slot, count) in slots {
        if left == 0 {
            break;
        }
        let taken = count.min(left);
        left -= taken;
        out.push((slot, count - taken));
    }
    out
}

/// The block to place and the item it is paid with: a block item held in the other hand,
/// else the clicked block itself.
fn material(
    host: &dyn Host,
    player: PlayerRef,
    hand: Hand,
    clicked: BlockState,
) -> Option<(BlockState, ItemKey)> {
    let other = match hand {
        Hand::Main => Hand::Off,
        Hand::Off => Hand::Main,
    };
    let offered = host
        .living(player.entity())
        .and_then(|l| l.hand_item(host, other))
        .and_then(|stack| {
            let block = host.item_block(stack.item())?;
            Some((host.block_state_default(block)?, stack.item()))
        });
    offered.or_else(|| Some((clicked, host.block_item(clicked.block())?)))
}

/// Whether a block can go at `pos`: its block is replaceable and no entity is in the way.
fn is_free(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> bool {
    let replaceable = level
        .block_state(host, pos)
        .and_then(|s| host.block_states().replaceable(s))
        .unwrap_or(false);
    let [x, y, z] = pos.map(f64::from);
    let crowded = level
        .entities_in(host, [x, y, z], [x + 1.0, y + 1.0, z + 1.0])
        .is_none_or(|e| e.iter().any(|&e| host.living(e).is_some()));
    replaceable && !crowded
}

fn build(host: &mut dyn ServerHost, event: &UseOnEvent<'_>, limit: usize) -> InteractionResult {
    let read: &dyn Host = &*host;
    let Some(player) = read.player(event.player) else {
        return InteractionResult::Pass;
    };
    let Some(level) = player.level(read) else {
        return InteractionResult::Pass;
    };
    let Some(clicked) = level.block_state(read, event.pos) else {
        return InteractionResult::Pass;
    };
    let Some((state, item)) = material(read, player, event.hand, clicked) else {
        return InteractionResult::Fail;
    };
    let creative = player.has_infinite_materials(read).unwrap_or(false);
    let slots: Vec<(u32, u32)> = player
        .inventory_slots(read)
        .unwrap_or_default()
        .into_iter()
        .filter(|(slot, s)| *slot <= 40 && s.item() == item && !s.is_damaged())
        .map(|(slot, s)| (slot, s.count()))
        .collect();
    let have = if creative {
        usize::MAX
    } else {
        slots.iter().map(|s| s.1 as usize).sum()
    };
    let block: BlockKey = clicked.block();
    let targets = plan(
        event.pos,
        event.face,
        limit.min(have),
        |p| level.block(read, p) == Some(block),
        |p| is_free(read, level, p),
    );
    let placed = targets
        .iter()
        .filter(|&&p| player.place_block(host, level, p, state, true).is_ok())
        .count() as u32;
    if placed == 0 {
        return InteractionResult::Fail;
    }
    if !creative {
        pay(host, player, &slots, placed);
        let slot = match event.hand {
            Hand::Main => EquipmentSlot::MainHand,
            Hand::Off => EquipmentSlot::OffHand,
        };
        if let Some(living) = host.living(player.entity()) {
            living.hurt_and_break(host, slot, placed as i32);
        }
    }
    if let Some(sound) = host.block_states().sound(state) {
        let [x, y, z] = add(event.pos, event.face).map(|c| f64::from(c) + 0.5);
        level.play_sound(
            host,
            &sound.place,
            [x, y, z],
            SoundSource::Blocks,
            (sound.volume + 1.0) / 2.0,
            sound.pitch * 0.8,
        );
    }
    InteractionResult::Success
}

/// Take `n` items from the slots planned from.
fn pay(host: &mut dyn ServerHost, player: PlayerRef, slots: &[(u32, u32)], n: u32) {
    for (slot, keep) in take_from(slots, n) {
        let stack = if keep == 0 {
            None
        } else {
            let Some(mut old) = read_stack(&*host, player, slot) else {
                continue;
            };
            if old.set_count(keep).is_err() {
                continue;
            }
            Some(old)
        };
        let _ = player.set_inventory_slot(host, slot, stack);
    }
}

fn read_stack(host: &dyn Host, player: PlayerRef, slot: u32) -> Option<ItemStack> {
    player
        .inventory_slots(host)?
        .into_iter()
        .find_map(|(s, stack)| (s == slot).then_some(stack))
}

/// One stick's behaviour: its placement limit.
struct Stick {
    limit: usize,
}

#[mod_api::horizon_part]
impl ItemPart for Stick {
    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        match host {
            HostSide::Server(host) => Some(build(host, event, self.limit)),
            // The client swings; the server decides what is placed.
            _ => Some(InteractionResult::Success),
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl mod_api::Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for (id, limit) in STICKS {
            let item = r.item_key(id);
            r.item_part(item, Stick { limit });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grows_the_connected_surface_one_layer_up() {
        // A 3x3 floor of matching blocks at y=0, clicked on top of its centre.
        let floor = |p: [i32; 3]| p[1] == 0 && p[0].abs() <= 1 && p[2].abs() <= 1;
        let mut got = plan([0, 0, 0], [0, 1, 0], 100, floor, |_| true);
        got.sort();
        let mut want: Vec<[i32; 3]> = (-1..=1)
            .flat_map(|x| (-1..=1).map(move |z| [x, 1, z]))
            .collect();
        want.sort();
        assert_eq!(got, want);
    }

    #[test]
    fn stops_at_the_limit_nearest_first() {
        // A long wall along x, clicked on its east face (+x) at the middle: the plane is y/z,
        // so a wall along x is one block of it.
        let row = |p: [i32; 3]| p[1] == 0 && p[2] == 0;
        let got = plan([0, 0, 0], [0, 0, 1], 3, row, |_| true);
        assert_eq!(got.len(), 3);
        assert_eq!(got[0], [0, 0, 1]);
        assert!(got[1..].iter().all(|p| p[0].abs() == 1));
    }

    #[test]
    fn a_blocked_front_neither_fills_nor_spreads() {
        // A row x = -3..3; the block in front of x = 1 is taken, cutting off x = 2 and 3.
        let row = |p: [i32; 3]| p[1] == 0 && p[2] == 0 && p[0].abs() <= 3;
        let free = |p: [i32; 3]| p != [1, 1, 0];
        let mut got = plan([0, 0, 0], [0, 1, 0], 100, row, free);
        got.sort();
        assert_eq!(got, vec![[-3, 1, 0], [-2, 1, 0], [-1, 1, 0], [0, 1, 0]]);
    }

    #[test]
    fn pays_from_the_lowest_slots_first() {
        let slots = [(2, 5), (7, 64), (30, 10)];
        assert_eq!(take_from(&slots, 20), vec![(2, 0), (7, 49)]);
        assert_eq!(take_from(&slots, 3), vec![(2, 2)]);
    }
}
