//! Which slots a player's loot lands in, apart from the game.

/// Slots of the loot chest (vanilla's 9x3 menu).
pub const SLOTS: usize = 27;

/// What opening the chest does for a player.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Opening {
    /// They opened it before: give back what they left.
    Restore,
    /// First time, and the chest has a loot table: roll it for them.
    Roll(String),
    /// First time, no loot table: an empty chest of their own.
    Empty,
}

/// Decides the opening from whether the player has a saved inventory and the chest's table.
pub fn opening(has_saved: bool, loot_table: Option<&str>) -> Opening {
    match (has_saved, loot_table.filter(|t| !t.is_empty())) {
        (true, _) => Opening::Restore,
        (false, Some(table)) => Opening::Roll(table.to_owned()),
        (false, None) => Opening::Empty,
    }
}

/// The slots `count` rolled stacks go to: distinct, shuffled as vanilla scatters loot, the same
/// for one player at one chest (`seed`: the player and the position mixed). More stacks than
/// slots get no slot: the rest are lost, as a full chest loses them in vanilla.
pub fn scatter(count: usize, slots: usize, seed: u64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..slots).collect();
    let mut state = seed | 1;
    for i in (1..slots).rev() {
        // xorshift64*, enough for a shuffle
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let r = state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33;
        order.swap(i, (r % (i as u64 + 1)) as usize);
    }
    order.truncate(count);
    order
}

/// A seed from the player's UUID and the chest's position.
pub fn seed(player: u128, pos: [i32; 3]) -> u64 {
    let mut h = (player as u64) ^ ((player >> 64) as u64).rotate_left(21);
    for c in pos {
        h = (h ^ (c as i64 as u64)).wrapping_mul(0x1000_0000_01B3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_players_first_open_rolls_and_every_later_one_restores() {
        assert_eq!(opening(false, Some("minecraft:chests/simple_dungeon")), Opening::Roll("minecraft:chests/simple_dungeon".into()));
        assert_eq!(opening(true, Some("minecraft:chests/simple_dungeon")), Opening::Restore);
        assert_eq!(opening(false, Some("")), Opening::Empty);
        assert_eq!(opening(false, None), Opening::Empty);
    }

    #[test]
    fn loot_lands_in_distinct_slots_of_the_chest() {
        let slots = scatter(12, SLOTS, seed(0xDEAD_BEEF, [10, 64, -3]));
        assert_eq!(slots.len(), 12);
        let mut sorted = slots.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 12);
        assert!(slots.iter().all(|s| *s < SLOTS));
    }

    #[test]
    fn each_player_gets_their_own_layout_and_the_same_one_every_time() {
        let a = seed(1, [0, 0, 0]);
        let b = seed(2, [0, 0, 0]);
        assert_eq!(scatter(10, SLOTS, a), scatter(10, SLOTS, a));
        assert_ne!(scatter(10, SLOTS, a), scatter(10, SLOTS, b));
        assert_eq!(scatter(40, SLOTS, a).len(), SLOTS);
    }
}
