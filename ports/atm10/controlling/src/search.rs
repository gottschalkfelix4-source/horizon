//! Controlling's list logic, apart from the game: which bindings conflict, which the search and
//! the filters keep, in which order, which keys no binding uses, and what a rebind or Reset All sets.

use mod_api::KeyConflictContext;

/// The key id of a binding bound to nothing.
pub const UNBOUND: &str = "key.keyboard.unknown";

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
    /// The bound key's id (`key.keyboard.space`).
    pub key_id: String,
    /// The modifiers held with it (GLFW bits).
    pub modifiers: u32,
    /// The key Reset puts it back on, and its modifiers.
    pub default_key: String,
    pub default_modifiers: u32,
    /// When it can conflict with another binding.
    pub context: KeyConflictContext,
    /// The mod that registered it; `None` for vanilla's and Horizon's.
    pub owner: Option<String>,
}

impl Binding {
    /// Whether it is on other than its default key.
    pub fn changed(&self) -> bool {
        self.key_id != self.default_key || self.modifiers != self.default_modifiers
    }
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
/// another debug key; `key.debug.modifier` is F3 itself and clashes with nothing. Otherwise their
/// conflict contexts must overlap.
fn same_context(a: &Binding, b: &Binding) -> bool {
    if a.id == "key.debug.modifier" || b.id == "key.debug.modifier" {
        return false;
    }
    a.id.starts_with("key.debug.") == b.id.starts_with("key.debug.")
        && a.context.conflicts(b.context)
}

/// For each binding, whether another binding shares its key and modifiers (vanilla's red key label).
pub fn conflicts(bindings: &[Binding]) -> Vec<bool> {
    bindings
        .iter()
        .enumerate()
        .map(|(i, a)| {
            !a.unbound
                && bindings.iter().enumerate().any(|(j, b)| {
                    i != j
                        && !b.unbound
                        && a.key_id == b.key_id
                        && a.modifiers == b.modifiers
                        && same_context(a, b)
                })
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

/// The ids of `keys` no bound binding uses, in order.
pub fn free_keys<'a>(keys: &'a [String], bindings: &[Binding]) -> Vec<&'a str> {
    keys.iter()
        .filter(|id| !bindings.iter().any(|b| !b.unbound && &b.key_id == *id))
        .map(|id| id.as_str())
        .collect()
}

/// The changes Reset All makes: every binding off its default key goes back, as `(id, key, modifiers)`.
pub fn reset_plan(bindings: &[Binding]) -> Vec<(String, String, u32)> {
    bindings
        .iter()
        .filter(|b| b.changed())
        .map(|b| (b.id.clone(), b.default_key.clone(), b.default_modifiers))
        .collect()
}

/// The key id of mouse button `button` (GLFW's numbering: 0 left, 1 right, 2 middle): `key.mouse.left`,
/// then `key.mouse.4` for the fourth button.
pub fn mouse_key(button: u32) -> String {
    match button {
        0 => "key.mouse.left".to_owned(),
        1 => "key.mouse.right".to_owned(),
        2 => "key.mouse.middle".to_owned(),
        n => format!("key.mouse.{}", n + 1),
    }
}

fn is_modifier_key(key: &str) -> bool {
    matches!(
        key,
        "key.keyboard.left.shift"
            | "key.keyboard.right.shift"
            | "key.keyboard.left.control"
            | "key.keyboard.right.control"
            | "key.keyboard.left.alt"
            | "key.keyboard.right.alt"
            | "key.keyboard.left.win"
            | "key.keyboard.right.win"
    )
}

/// What a rebind to `key` with `held` modifiers sets on `binding`: only a mod's binding keeps shift,
/// control and alt, and a modifier key on its own is bound bare.
pub fn rebind(binding: &Binding, key: &str, held: u32) -> (String, u32) {
    let modifiers = if binding.owner.is_some() && !is_modifier_key(key) {
        held & 7
    } else {
        0
    };
    (key.to_owned(), modifiers)
}

/// A key as Controls shows it: the modifiers' names, then `key_name`. `modifier_name` gives the
/// name of `"shift"`, `"control"` or `"alt"`.
pub fn key_label(key_name: &str, modifiers: u32, modifier_name: impl Fn(&str) -> String) -> String {
    let mut label = String::new();
    for (bit, name) in [(2, "control"), (4, "alt"), (1, "shift")] {
        if modifiers & bit != 0 {
            label.push_str(&modifier_name(name));
            label.push_str(" + ");
        }
    }
    label.push_str(key_name);
    label
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
            key_id: if key == "Not Bound" {
                UNBOUND.into()
            } else {
                format!("key.keyboard.{}", key.to_lowercase())
            },
            modifiers: 0,
            default_key: UNBOUND.into(),
            default_modifiers: 0,
            context: KeyConflictContext::Universal,
            owner: None,
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
        let keys: Vec<String> = ["a", "k", "q", "z"]
            .into_iter()
            .map(|k| format!("key.keyboard.{k}"))
            .collect();
        assert_eq!(
            free_keys(&keys, &sample()),
            ["key.keyboard.k", "key.keyboard.z"]
        );
    }

    #[test]
    fn same_key_with_other_modifiers_or_non_overlapping_context_is_no_conflict() {
        let mut list = vec![
            b("a:one", "One", "x", "K"),
            b("a:two", "Two", "x", "K"),
            b("a:three", "Three", "x", "K"),
        ];
        list[1].modifiers = 2; // Ctrl+K
        assert_eq!(conflicts(&list), [true, false, true]);
        list[0].context = KeyConflictContext::InGame;
        list[2].context = KeyConflictContext::Gui;
        assert_eq!(conflicts(&list), [false, false, false]);
    }

    #[test]
    fn reset_all_puts_only_the_changed_bindings_back_on_their_defaults() {
        let mut list = sample();
        list[0].default_key = "key.keyboard.space".into(); // already there
        list[1].default_key = "key.keyboard.x".into(); // moved to Q
        list[3].default_key = "key.keyboard.k".into(); // unbound by the player
        list[3].default_modifiers = 2;
        assert_eq!(
            reset_plan(&list)
                .into_iter()
                .filter(|(id, ..)| id != "key.mymod.dash"
                    && id != "key.left"
                    && id != "key.debug.reloadChunk")
                .collect::<Vec<_>>(),
            [
                ("key.drop".to_owned(), "key.keyboard.x".to_owned(), 0),
                (
                    "key.smoothCamera".to_owned(),
                    "key.keyboard.k".to_owned(),
                    2
                ),
            ]
        );
    }

    #[test]
    fn a_rebind_keeps_modifiers_for_mod_bindings_only_and_never_on_a_modifier_key() {
        let vanilla = sample().remove(0);
        let mut modded = vanilla.clone();
        modded.owner = Some("zoom".into());
        assert_eq!(
            rebind(&vanilla, "key.keyboard.k", 3),
            ("key.keyboard.k".into(), 0)
        );
        assert_eq!(
            rebind(&modded, "key.keyboard.k", 3),
            ("key.keyboard.k".into(), 1 | 2)
        );
        assert_eq!(
            rebind(&modded, "key.keyboard.left.control", 2),
            ("key.keyboard.left.control".into(), 0)
        );
    }

    #[test]
    fn mouse_buttons_name_like_the_game_and_labels_list_modifiers_first() {
        assert_eq!(
            [
                mouse_key(0),
                mouse_key(1),
                mouse_key(2),
                mouse_key(3),
                mouse_key(7)
            ],
            [
                "key.mouse.left",
                "key.mouse.right",
                "key.mouse.middle",
                "key.mouse.4",
                "key.mouse.8"
            ]
        );
        assert_eq!(
            key_label("K", 7, |m| m.to_uppercase()),
            "CONTROL + ALT + SHIFT + K"
        );
        assert_eq!(key_label("K", 0, |m| m.to_uppercase()), "K");
    }
}
