//! Controlling's list logic, apart from the game: which bindings conflict, which the search and
//! the filters keep, in which order, and which keys no binding uses.

/// One binding as the list shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    /// The mapping's id, its translation key (`key.jump`).
    pub id: String,
    /// Its name in the player's language ("Jump").
    pub name: String,
    /// Its category's id (`movement`).
    pub category: String,
    /// The category's name in the player's language ("Movement").
    pub category_name: String,
    /// The bound key as Controls shows it ("Space"), modifiers included.
    pub key: String,
    /// Bound to nothing.
    pub unbound: bool,
}

/// Which bindings the list shows (Controlling's "Show" button).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Show {
    #[default]
    All,
    Conflicting,
    Unbound,
}

impl Show {
    pub fn next(self) -> Self {
        match self {
            Show::All => Show::Conflicting,
            Show::Conflicting => Show::Unbound,
            Show::Unbound => Show::All,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Show::All => "controlling.show.all",
            Show::Conflicting => "controlling.show.conflicts",
            Show::Unbound => "controlling.show.unbound",
        }
    }
}

/// What the search box matches against (Controlling's "Search" toggle).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchBy {
    #[default]
    Name,
    Key,
    Both,
}

impl SearchBy {
    pub fn next(self) -> Self {
        match self {
            SearchBy::Name => SearchBy::Key,
            SearchBy::Key => SearchBy::Both,
            SearchBy::Both => SearchBy::Name,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            SearchBy::Name => "controlling.search_by.name",
            SearchBy::Key => "controlling.search_by.key",
            SearchBy::Both => "controlling.search_by.both",
        }
    }
}

/// The list's order (Controlling's "Sort" button). `None` keeps the game's order, by category.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    None,
    AToZ,
    ZToA,
}

impl Sort {
    pub fn next(self) -> Self {
        match self {
            Sort::None => Sort::AToZ,
            Sort::AToZ => Sort::ZToA,
            Sort::ZToA => Sort::None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Sort::None => "controlling.sort.none",
            Sort::AToZ => "controlling.sort.a_z",
            Sort::ZToA => "controlling.sort.z_a",
        }
    }
}

/// The search box and the three buttons.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    pub text: String,
    pub by: SearchBy,
    pub show: Show,
    pub sort: Sort,
}

/// Whether two bindings can clash: a debug key only acts with F3 held, so it clashes only with
/// another debug key; `key.debug.modifier` is F3 itself and clashes with nothing.
fn same_context(a: &Binding, b: &Binding) -> bool {
    if a.id == "key.debug.modifier" || b.id == "key.debug.modifier" {
        return false;
    }
    (a.category == "debug") == (b.category == "debug")
}

/// For each binding, whether another binding shares its key (vanilla's red key label).
pub fn conflicts(bindings: &[Binding]) -> Vec<bool> {
    bindings
        .iter()
        .enumerate()
        .map(|(i, a)| {
            !a.unbound
                && bindings
                    .iter()
                    .enumerate()
                    .any(|(j, b)| i != j && !b.unbound && a.key == b.key && same_context(a, b))
        })
        .collect()
}

fn contains(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(needle)
}

/// The indices of `bindings` the list shows for `query`, in the order it shows them.
pub fn filter(bindings: &[Binding], query: &Query) -> Vec<usize> {
    let clash = conflicts(bindings);
    let needle = query.text.trim().to_lowercase();
    let mut shown: Vec<usize> = (0..bindings.len())
        .filter(|&i| {
            let b = &bindings[i];
            let kept = match query.show {
                Show::All => true,
                Show::Conflicting => clash[i],
                Show::Unbound => b.unbound,
            };
            let found = needle.is_empty()
                || match query.by {
                    SearchBy::Name => contains(&b.name, &needle),
                    SearchBy::Key => contains(&b.key, &needle),
                    SearchBy::Both => contains(&b.name, &needle) || contains(&b.key, &needle),
                };
            kept && found
        })
        .collect();
    match query.sort {
        Sort::None => {}
        Sort::AToZ => shown.sort_by_key(|&i| bindings[i].name.to_lowercase()),
        Sort::ZToA => {
            shown.sort_by_key(|&i| bindings[i].name.to_lowercase());
            shown.reverse();
        }
    }
    shown
}

/// The keys of `keys` (`(id, name as Controls shows it)`) no bound binding uses, in order.
pub fn free_keys<'a>(keys: &'a [(String, String)], bindings: &[Binding]) -> Vec<&'a str> {
    keys.iter()
        .filter(|(_, name)| !bindings.iter().any(|b| !b.unbound && &b.key == name))
        .map(|(id, _)| id.as_str())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(id: &str, name: &str, category: &str, key: &str) -> Binding {
        Binding {
            id: id.into(),
            name: name.into(),
            category: category.into(),
            category_name: category.into(),
            key: key.into(),
            unbound: key == "Not Bound",
        }
    }

    fn sample() -> Vec<Binding> {
        vec![
            b("key.jump", "Jump", "movement", "Space"),
            b("key.drop", "Drop Selected Item", "inventory", "Q"),
            b("key.mymod.dash", "Dash", "movement", "Space"),
            b(
                "key.smoothCamera",
                "Toggle Cinematic Camera",
                "misc",
                "Not Bound",
            ),
            b("key.debug.reloadChunk", "Reload Chunks", "debug", "A"),
            b("key.left", "Strafe Left", "movement", "A"),
        ]
    }

    #[test]
    fn two_bindings_on_one_key_conflict_but_unbound_ones_never_do() {
        let mut list = sample();
        list.push(b(
            "key.spectatorOutlines",
            "Highlight Players",
            "spectator",
            "Not Bound",
        ));
        let clash = conflicts(&list);
        assert!(clash[0] && clash[2], "jump and dash share Space");
        assert!(!clash[1]);
        assert!(
            !clash[3] && !clash[6],
            "two unbound bindings are no conflict"
        );
    }

    #[test]
    fn a_debug_key_does_not_conflict_with_a_gameplay_key() {
        let clash = conflicts(&sample());
        assert!(!clash[4] && !clash[5], "F3+A and A are different presses");
    }

    #[test]
    fn search_by_name_ignores_case_and_search_by_key_reads_the_key() {
        let list = sample();
        let by_name = Query {
            text: "  JUMP ".into(),
            ..Query::default()
        };
        assert_eq!(filter(&list, &by_name), vec![0]);
        let by_key = Query {
            text: "space".into(),
            by: SearchBy::Key,
            ..Query::default()
        };
        assert_eq!(filter(&list, &by_key), vec![0, 2]);
        let both = Query {
            text: "q".into(),
            by: SearchBy::Both,
            ..Query::default()
        };
        // "Drop Selected Item" holds no q, but its key is Q.
        assert_eq!(filter(&list, &both), vec![1]);
    }

    #[test]
    fn show_conflicting_and_unbound_filter_the_list() {
        let list = sample();
        let conflicting = Query {
            show: Show::Conflicting,
            ..Query::default()
        };
        assert_eq!(filter(&list, &conflicting), vec![0, 2]);
        let unbound = Query {
            show: Show::Unbound,
            ..Query::default()
        };
        assert_eq!(filter(&list, &unbound), vec![3]);
    }

    #[test]
    fn sorting_orders_by_name_both_ways() {
        let list = sample();
        let az = Query {
            sort: Sort::AToZ,
            ..Query::default()
        };
        let names: Vec<&str> = filter(&list, &az)
            .into_iter()
            .map(|i| list[i].name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "Dash",
                "Drop Selected Item",
                "Jump",
                "Reload Chunks",
                "Strafe Left",
                "Toggle Cinematic Camera"
            ]
        );
        let za = Query {
            sort: Sort::ZToA,
            ..Query::default()
        };
        assert_eq!(filter(&list, &za)[0], 3);
    }

    #[test]
    fn free_keys_are_the_ones_no_bound_binding_uses() {
        let keys: Vec<(String, String)> = [
            ("key.keyboard.a", "A"),
            ("key.keyboard.k", "K"),
            ("key.keyboard.q", "Q"),
            ("key.keyboard.z", "Z"),
        ]
        .into_iter()
        .map(|(a, b)| (a.to_owned(), b.to_owned()))
        .collect();
        assert_eq!(
            free_keys(&keys, &sample()),
            ["key.keyboard.k", "key.keyboard.z"]
        );
    }
}
