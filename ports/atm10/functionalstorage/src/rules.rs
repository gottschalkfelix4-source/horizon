//! The drawers' counting, apart from the game.

/// The woods the port declares drawers in.
pub const WOODS: [&str; 6] = ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak"];
/// Slots a drawer can have.
pub const SIZES: [usize; 3] = [1, 2, 4];

/// How many drawer slots (1, 2 or 4) a block `functionalstorage:<wood>_<n>` has.
pub fn slots_of(block: &str) -> Option<usize> {
    let name = block.strip_prefix("functionalstorage:")?;
    let (wood, n) = name.rsplit_once('_')?;
    let n: usize = n.parse().ok()?;
    (WOODS.contains(&wood) && SIZES.contains(&n)).then_some(n)
}

/// Stacks one slot of a drawer with `slots` slots holds: 32, 16 or 8.
pub fn stacks_per_slot(slots: usize) -> u32 {
    match slots {
        1 => 32,
        2 => 16,
        _ => 8,
    }
}

/// What a drawer slot holds: a kind of item and how many.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slots<K> {
    pub slots: Vec<Option<(K, u32)>>,
}

impl<K: PartialEq + Clone> Slots<K> {
    pub fn new(n: usize) -> Self {
        Slots { slots: vec![None; n] }
    }

    /// Puts up to `amount` of `key` in slot `i`, which holds at most `cap` of it: how many went in.
    /// A slot holds one kind; a kind already in another slot is not taken, so one kind never
    /// spreads over two slots.
    pub fn insert(&mut self, i: usize, key: &K, amount: u32, cap: u32) -> u32 {
        if self.slots.iter().enumerate().any(|(j, s)| j != i && s.as_ref().is_some_and(|(k, _)| k == key)) {
            return 0;
        }
        let Some(slot) = self.slots.get_mut(i) else { return 0 };
        match slot {
            None => {
                let put = amount.min(cap);
                if put > 0 {
                    *slot = Some((key.clone(), put));
                }
                put
            }
            Some((k, n)) if k == key => {
                let put = amount.min(cap.saturating_sub(*n));
                *n += put;
                put
            }
            Some(_) => 0,
        }
    }

    /// Takes up to `amount` of `key` out of slot `i`: how many came out. An emptied slot is free
    /// again.
    pub fn extract(&mut self, i: usize, key: &K, amount: u32) -> u32 {
        let Some(slot) = self.slots.get_mut(i) else { return 0 };
        match slot {
            Some((k, n)) if k == key => {
                let took = amount.min(*n);
                *n -= took;
                if *n == 0 {
                    *slot = None;
                }
                took
            }
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawers_are_named_by_wood_and_slot_count() {
        assert_eq!(slots_of("functionalstorage:oak_2"), Some(2));
        assert_eq!(slots_of("functionalstorage:dark_oak_4"), Some(4));
        assert_eq!(slots_of("functionalstorage:oak_3"), None);
        assert_eq!(slots_of("functionalstorage:cherry_1"), None);
        assert_eq!(slots_of("minecraft:oak_1"), None);
    }

    #[test]
    fn a_single_drawer_holds_32_stacks_and_a_four_slot_drawer_8_per_slot() {
        let cap = stacks_per_slot(1) * 64;
        let mut one = Slots::new(1);
        assert_eq!(one.insert(0, &"cobble", 5000, cap), 2048);
        assert_eq!(one.insert(0, &"cobble", 5000, cap), 0);
        assert_eq!(stacks_per_slot(4) * 64, 512);
        assert_eq!(stacks_per_slot(2) * 16, 256); // ender pearls stack to 16
    }

    #[test]
    fn a_slot_holds_one_kind_and_frees_up_when_empty() {
        let mut d = Slots::new(2);
        assert_eq!(d.insert(0, &"stone", 10, 100), 10);
        assert_eq!(d.insert(0, &"dirt", 10, 100), 0);
        assert_eq!(d.extract(0, &"dirt", 5), 0);
        assert_eq!(d.extract(0, &"stone", 25), 10);
        assert_eq!(d.slots[0], None);
        assert_eq!(d.insert(0, &"dirt", 3, 100), 3);
    }

    #[test]
    fn a_kind_never_takes_two_slots() {
        let mut d = Slots::new(2);
        d.insert(0, &"stone", 1, 100);
        assert_eq!(d.insert(1, &"stone", 5, 100), 0);
        assert_eq!(d.insert(1, &"dirt", 5, 100), 5);
    }
}
