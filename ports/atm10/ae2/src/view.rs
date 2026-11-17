//! The ME terminal's view, free of the game: which network items show on a page, how a search
//! filters them, and what each slot of the six-row view is. Rows one to five list items (45 slots);
//! the last row holds the controls.

pub const PER_PAGE: usize = 45;
pub const PREV: usize = 45;
pub const SEARCH: usize = 49;
pub const NEXT: usize = 53;
pub const ROWS: u8 = 6;

/// The items of the network (id, count), the most first, equal counts by id.
pub fn listing(mut all: Vec<(String, u64)>) -> Vec<(String, u64)> {
    all.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    all
}

/// The items whose id (without its namespace) contains `search`, ignoring case; all for none.
pub fn filter(items: &[(String, u64)], search: Option<&str>) -> Vec<(String, u64)> {
    match search.map(str::to_lowercase).filter(|s| !s.is_empty()) {
        None => items.to_vec(),
        Some(s) => items
            .iter()
            .filter(|(id, _)| id.split_once(':').map_or(id.as_str(), |(_, p)| p).replace('_', " ").contains(&s.replace('_', " ")))
            .cloned()
            .collect(),
    }
}

/// How many pages `len` items take (at least one).
pub fn pages(len: usize) -> usize {
    len.div_ceil(PER_PAGE).max(1)
}

/// The items on `page` (clamped to the last).
pub fn page_of(items: &[(String, u64)], page: usize) -> (usize, &[(String, u64)]) {
    let page = page.min(pages(items.len()) - 1);
    let from = page * PER_PAGE;
    (page, &items[from..items.len().min(from + PER_PAGE)])
}

/// What a click on a slot of the view does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    Item(usize),
    Prev,
    Next,
    Search,
    Blank,
    Player(usize),
}

pub fn slot_kind(index: usize) -> Slot {
    match index {
        i if i < PER_PAGE => Slot::Item(i),
        PREV => Slot::Prev,
        NEXT => Slot::Next,
        SEARCH => Slot::Search,
        i if i < 54 => Slot::Blank,
        i => Slot::Player(i),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<(String, u64)> {
        listing(vec![
            ("minecraft:dirt".into(), 5),
            ("minecraft:oak_log".into(), 900),
            ("ae2:certus_quartz_crystal".into(), 5),
        ])
    }

    #[test]
    fn the_most_stored_comes_first_and_ties_go_by_id() {
        let ids: Vec<_> = sample().into_iter().map(|(i, _)| i).collect();
        assert_eq!(ids, ["minecraft:oak_log", "ae2:certus_quartz_crystal", "minecraft:dirt"]);
    }

    #[test]
    fn a_search_matches_names_not_namespaces() {
        let s = sample();
        assert_eq!(filter(&s, Some("QUARTZ")).len(), 1);
        assert_eq!(filter(&s, Some("oak log")).len(), 1);
        assert_eq!(filter(&s, Some("minecraft")).len(), 0);
        assert_eq!(filter(&s, None).len(), 3);
        assert_eq!(filter(&s, Some("")).len(), 3);
    }

    #[test]
    fn pages_hold_45_and_clamp() {
        let many: Vec<(String, u64)> = (0..100).map(|i| (format!("a:i{i:03}"), 1)).collect();
        assert_eq!(pages(0), 1);
        assert_eq!(pages(100), 3);
        let (p, items) = page_of(&many, 9);
        assert_eq!((p, items.len()), (2, 10));
        assert_eq!(page_of(&many, 0).1.len(), 45);
    }

    #[test]
    fn slots_map_to_controls() {
        assert_eq!(slot_kind(0), Slot::Item(0));
        assert_eq!(slot_kind(44), Slot::Item(44));
        assert_eq!(slot_kind(45), Slot::Prev);
        assert_eq!(slot_kind(49), Slot::Search);
        assert_eq!(slot_kind(53), Slot::Next);
        assert_eq!(slot_kind(47), Slot::Blank);
        assert_eq!(slot_kind(54), Slot::Player(54));
    }
}
