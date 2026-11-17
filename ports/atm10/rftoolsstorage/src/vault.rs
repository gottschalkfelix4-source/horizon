//! The storage a Modular Storage's modules add up to: a list of item kinds with counts, whose
//! stacks fill a number of slots (a kind of `count` items with stacks of `max` uses
//! `ceil(count / max)` slots). Independent of the game: `K` is whatever tells kinds apart.

/// One kind of item and how many of it are stored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry<K> {
    pub kind: K,
    /// The kind's stack size, which decides the slots it takes.
    pub max: u32,
    pub count: u64,
}

impl<K> Entry<K> {
    pub fn slots(&self) -> u64 {
        self.count.div_ceil(u64::from(self.max.max(1)))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vault<K> {
    pub entries: Vec<Entry<K>>,
}

impl<K> Default for Vault<K> {
    fn default() -> Self {
        Self { entries: Vec::new() }
    }
}

impl<K: Clone> Vault<K> {
    pub fn used_slots(&self) -> u64 {
        self.entries.iter().map(Entry::slots).sum()
    }

    /// How many of a kind (`max` per stack) fit when `capacity` slots exist: what the stacks
    /// already there leave free, plus the empty slots.
    pub fn room(&self, capacity: u64, max: u32, same: impl Fn(&K) -> bool) -> u64 {
        let max = u64::from(max.max(1));
        let free = capacity.saturating_sub(self.used_slots());
        let slack = self
            .entries
            .iter()
            .find(|e| same(&e.kind))
            .map_or(0, |e| e.slots() * max - e.count);
        slack + free * max
    }

    /// Stores up to `n` of `kind`: how many were taken.
    pub fn insert(&mut self, kind: &K, max: u32, n: u32, capacity: u64, same: impl Fn(&K) -> bool) -> u32 {
        let take = u64::from(n).min(self.room(capacity, max, &same));
        if take == 0 {
            return 0;
        }
        match self.entries.iter_mut().find(|e| same(&e.kind)) {
            Some(e) => e.count += take,
            None => self.entries.push(Entry { kind: kind.clone(), max, count: take }),
        }
        take as u32
    }

    /// Takes up to `n` of the entry at `index` (never more than one stack's worth at a time is
    /// the caller's business): how many came out. An emptied kind leaves the list.
    pub fn take(&mut self, index: usize, n: u32) -> u32 {
        let Some(e) = self.entries.get_mut(index) else {
            return 0;
        };
        let out = u64::from(n).min(e.count);
        e.count -= out;
        if e.count == 0 {
            self.entries.remove(index);
        }
        out as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stacks_fill_slots_and_a_kind_tops_up_its_last_stack_first() {
        let mut v: Vault<u8> = Vault::default();
        // Two slots of 64: 100 apple-likes take both.
        assert_eq!(v.insert(&1, 64, 100, 2, |k| *k == 1), 100);
        assert_eq!(v.used_slots(), 2);
        // 28 more fit in the partly filled second stack; a second kind finds no slot.
        assert_eq!(v.room(2, 64, |k| *k == 1), 28);
        assert_eq!(v.insert(&2, 64, 5, 2, |k| *k == 2), 0);
        assert_eq!(v.insert(&1, 64, 50, 2, |k| *k == 1), 28);
        assert_eq!(v.entries[0].count, 128);
    }

    #[test]
    fn unstackable_items_use_one_slot_each() {
        let mut v: Vault<u8> = Vault::default();
        assert_eq!(v.insert(&7, 1, 10, 3, |k| *k == 7), 3);
        assert_eq!(v.used_slots(), 3);
    }

    #[test]
    fn taking_everything_removes_the_kind_and_frees_its_slots() {
        let mut v: Vault<u8> = Vault::default();
        v.insert(&1, 16, 20, 4, |k| *k == 1);
        v.insert(&2, 16, 3, 4, |k| *k == 2);
        assert_eq!(v.take(0, 12), 12);
        assert_eq!(v.entries[0].count, 8);
        assert_eq!(v.take(0, 99), 8);
        assert_eq!(v.entries.len(), 1);
        assert_eq!(v.entries[0].kind, 2);
        assert_eq!(v.used_slots(), 1);
    }
}
