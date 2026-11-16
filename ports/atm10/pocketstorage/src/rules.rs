//! Sorting a container's stacks, apart from the game.

/// The three containers: block path, slots and the vanilla menu that shows them.
pub const TIERS: [(&str, usize, &str); 3] = [
    ("pocket_pouch", 9, "minecraft:generic_9x1"),
    ("pocket_chest", 27, "minecraft:generic_9x3"),
    ("pocket_vault", 54, "minecraft:generic_9x6"),
];

/// Packs `stacks` as a sort button does: stacks of one kind merge up to `max` each, kinds go in
/// order of `name` (then by how the original stacks came), and the empty slots are at the end.
/// `slots` is how many the container has: the result is always that long.
pub fn compact<K: PartialEq + Clone>(
    stacks: Vec<Option<(K, u32)>>,
    slots: usize,
    name: impl Fn(&K) -> String,
    max: impl Fn(&K) -> u32,
) -> Vec<Option<(K, u32)>> {
    let mut kinds: Vec<(K, u32)> = Vec::new();
    for (k, n) in stacks.into_iter().flatten() {
        match kinds.iter_mut().find(|(have, _)| *have == k) {
            Some((_, total)) => *total += n,
            None => kinds.push((k, n)),
        }
    }
    kinds.sort_by_key(|(k, _)| name(k));
    let mut out = Vec::new();
    for (k, mut total) in kinds {
        let per = max(&k).max(1);
        while total > 0 {
            let n = total.min(per);
            out.push(Some((k.clone(), n)));
            total -= n;
        }
    }
    out.resize(slots, None);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn max(k: &&str) -> u32 {
        if *k == "pearl" { 16 } else { 64 }
    }

    #[test]
    fn sorting_merges_partial_stacks_and_orders_by_name() {
        let items = vec![Some(("stone", 40)), None, Some(("dirt", 10)), Some(("stone", 40)), Some(("dirt", 5))];
        let sorted = compact(items, 6, |k| k.to_string(), max);
        assert_eq!(
            sorted,
            vec![Some(("dirt", 15)), Some(("stone", 64)), Some(("stone", 16)), None, None, None]
        );
    }

    #[test]
    fn sorting_respects_each_kinds_stack_size_and_keeps_every_item() {
        let items = vec![Some(("pearl", 16)), Some(("pearl", 16)), Some(("pearl", 3))];
        let sorted = compact(items, 4, |k| k.to_string(), max);
        assert_eq!(sorted, vec![Some(("pearl", 16)), Some(("pearl", 16)), Some(("pearl", 3)), None]);
        let total: u32 = sorted.iter().flatten().map(|(_, n)| n).sum();
        assert_eq!(total, 35);
    }
}
