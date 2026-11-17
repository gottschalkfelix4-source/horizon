//! KeybindsPurger's choices, apart from the game: which bindings are purged, saved as a list with the
//! key each one had, and the steps (unbind, hide, restore) the game must take to match.

use std::collections::BTreeMap;

use mod_api::KeyConflictContext;

/// The key id of a binding bound to nothing.
pub const UNBOUND: &str = "key.keyboard.unknown";

/// One binding as `ClientHost::key_bindings` lists it, reduced to what purging needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    /// `key.jump`, or `<mod id>:<id>`.
    pub id: String,
    /// The key now bound (`key.keyboard.k`), `UNBOUND` for none.
    pub key_id: String,
    /// The modifiers held with it (GLFW bits).
    pub modifiers: u32,
    /// What Reset puts back.
    pub default_key: String,
    pub default_modifiers: u32,
    pub context: KeyConflictContext,
    /// The mod that registered it; `None` for vanilla's and Horizon's.
    pub owner: Option<String>,
}

impl Binding {
    pub fn bound(&self) -> bool {
        self.key_id != UNBOUND
    }

    /// Whether `self` and `other` would both act on one press: same key and modifiers, overlapping
    /// contexts, and debug keys (F3 + key) only among themselves.
    fn clashes_with(&self, other: &Binding) -> bool {
        self.bound()
            && self.key_id == other.key_id
            && self.modifiers == other.modifiers
            && self.context.conflicts(other.context)
            && self.id.starts_with("key.debug.") == other.id.starts_with("key.debug.")
    }
}

/// One change to the game's bindings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Rebind `id` to `key` with `modifiers` (`UNBOUND` unbinds).
    Rebind(String, String, u32),
    /// Hide `id` from Controls, or show it again.
    Hide(String, bool),
}

/// The purged bindings' ids, each with the key it had when purged (`None`: it was unbound, or the
/// list was written by hand).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Purged(pub BTreeMap<String, Option<(String, u32)>>);

impl Purged {
    /// Reads the saved list: `id`, or `id<TAB>key<TAB>modifiers`, per line; blank lines and `#`
    /// comments are skipped.
    pub fn parse(text: &str) -> Self {
        let mut list = BTreeMap::new();
        for line in text.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.split('\t');
            let id = parts.next().unwrap_or_default().trim().to_owned();
            let original = match (
                parts.next(),
                parts.next().and_then(|m| m.trim().parse().ok()),
            ) {
                (Some(key), Some(modifiers)) => Some((key.trim().to_owned(), modifiers)),
                _ => None,
            };
            list.insert(id, original);
        }
        Purged(list)
    }

    /// The list as it is saved.
    pub fn write(&self) -> String {
        let mut out = String::from(
            "# KeybindsPurger: one purged key binding per line, optionally with the key it had (id<TAB>key<TAB>modifiers)\n",
        );
        for (id, original) in &self.0 {
            out.push_str(id);
            if let Some((key, modifiers)) = original {
                out.push_str(&format!("\t{key}\t{modifiers}"));
            }
            out.push('\n');
        }
        out
    }

    pub fn contains(&self, id: &str) -> bool {
        self.0.contains_key(id)
    }

    /// Purges `binding`: remembers its key, and returns the steps that unbind and hide it.
    pub fn purge(&mut self, binding: &Binding) -> Vec<Step> {
        if self.contains(&binding.id) {
            return Vec::new();
        }
        let original = binding
            .bound()
            .then(|| (binding.key_id.clone(), binding.modifiers));
        self.0.insert(binding.id.clone(), original);
        let mut steps = Vec::new();
        if binding.bound() {
            steps.push(Step::Rebind(binding.id.clone(), UNBOUND.to_owned(), 0));
        }
        steps.push(Step::Hide(binding.id.clone(), true));
        steps
    }

    /// Keeps `binding` again: shows it and puts its remembered key back (its default when none was
    /// remembered).
    pub fn keep(&mut self, binding: &Binding) -> Vec<Step> {
        let Some(original) = self.0.remove(&binding.id) else {
            return Vec::new();
        };
        let (key, modifiers) =
            original.unwrap_or_else(|| (binding.default_key.clone(), binding.default_modifiers));
        vec![
            Step::Rebind(binding.id.clone(), key, modifiers),
            Step::Hide(binding.id.clone(), false),
        ]
    }

    /// Purges every binding that shares its key with an earlier one, so each key keeps the first
    /// binding in Controls' order (vanilla's before mods'). Bindings already purged don't count
    /// as holding their key.
    pub fn purge_conflicts(&mut self, bindings: &[Binding]) -> Vec<Step> {
        let mut held: Vec<&Binding> = Vec::new();
        let mut steps = Vec::new();
        for b in bindings {
            if self.contains(&b.id) || !b.bound() {
                continue;
            }
            if held.iter().any(|h| h.clashes_with(b)) {
                steps.extend(self.purge(b));
            } else {
                held.push(b);
            }
        }
        steps
    }

    /// Purges every binding a mod registered, except those of `own_mod` (so its own key stays).
    pub fn purge_modded(&mut self, bindings: &[Binding], own_mod: &str) -> Vec<Step> {
        let mut steps = Vec::new();
        for b in bindings {
            if b.owner.as_deref().is_some_and(|o| o != own_mod) {
                steps.extend(self.purge(b));
            }
        }
        steps
    }

    /// Keeps every binding again.
    pub fn keep_all(&mut self, bindings: &[Binding]) -> Vec<Step> {
        let ids: Vec<String> = self.0.keys().cloned().collect();
        let mut steps = Vec::new();
        for id in ids {
            match bindings.iter().find(|b| b.id == id) {
                Some(b) => steps.extend(self.keep(b)),
                None => {
                    self.0.remove(&id);
                }
            }
        }
        steps
    }

    /// The steps that put the game right again: a purged binding the player bound to a key since (or
    /// that a mod added after the purge) is unbound.
    pub fn enforce(&self, bindings: &[Binding]) -> Vec<Step> {
        bindings
            .iter()
            .filter(|b| b.bound() && self.contains(&b.id))
            .map(|b| Step::Rebind(b.id.clone(), UNBOUND.to_owned(), 0))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(id: &str, key: &str) -> Binding {
        Binding {
            id: id.into(),
            key_id: if key.is_empty() {
                UNBOUND.into()
            } else {
                format!("key.keyboard.{key}")
            },
            modifiers: 0,
            default_key: "key.keyboard.x".into(),
            default_modifiers: 0,
            context: KeyConflictContext::Universal,
            owner: id.split_once(':').map(|(m, _)| m.to_owned()),
        }
    }

    fn ids(steps: &[Step]) -> Vec<&str> {
        steps
            .iter()
            .filter_map(|s| match s {
                Step::Rebind(id, ..) => Some(id.as_str()),
                Step::Hide(..) => None,
            })
            .collect()
    }

    #[test]
    fn the_saved_list_keeps_each_original_key_and_reads_old_lists() {
        let mut purged = Purged::default();
        let mut ctrl_k = b("zoom:in", "k");
        ctrl_k.modifiers = 2;
        purged.purge(&ctrl_k);
        purged.purge(&b("key.advancements", ""));
        assert_eq!(Purged::parse(&purged.write()), purged);
        assert_eq!(purged.0["zoom:in"], Some(("key.keyboard.k".to_owned(), 2)));
        let edited = Purged::parse("# mine\n\n  key.chat  \nkey.drop\tkey.keyboard.q\t0\n");
        assert_eq!(edited.0["key.chat"], None);
        assert_eq!(edited.0["key.drop"], Some(("key.keyboard.q".to_owned(), 0)));
    }

    #[test]
    fn purging_conflicts_keeps_the_first_binding_of_each_key_and_modifiers() {
        let mut ctrl_space = b("othermod:fly", "space");
        ctrl_space.modifiers = 2;
        let bindings = [
            b("key.drop", "q"),
            b("key.jump", "space"),
            b("mymod:dash", "space"),
            ctrl_space,
            b("mymod:menu", "q"),
            b("key.smoothCamera", ""),
        ];
        let mut purged = Purged::default();
        let steps = purged.purge_conflicts(&bindings);
        assert_eq!(ids(&steps), ["mymod:dash", "mymod:menu"]);
        assert!(purged.contains("mymod:dash") && !purged.contains("othermod:fly"));
        // A second pass finds nothing more.
        assert!(purged.purge_conflicts(&bindings).is_empty());
    }

    #[test]
    fn a_purged_first_binding_lets_the_next_one_keep_the_key() {
        let bindings = [b("key.jump", "space"), b("mymod:dash", "space")];
        let mut purged = Purged::parse("key.jump");
        purged.purge_conflicts(&bindings);
        assert!(!purged.contains("mymod:dash"));
    }

    #[test]
    fn purge_all_modded_takes_other_mods_keys_but_neither_vanillas_nor_its_own() {
        let bindings = [
            b("key.jump", "space"),
            b("zoom:in", "z"),
            b("keybindspurger:open", ""),
            b("mymod:dash", ""),
        ];
        let mut purged = Purged::default();
        let steps = purged.purge_modded(&bindings, "keybindspurger");
        assert_eq!(
            purged.0.keys().collect::<Vec<_>>(),
            ["mymod:dash", "zoom:in"]
        );
        // Only the bound one needs unbinding; both are hidden.
        assert_eq!(ids(&steps), ["zoom:in"]);
        assert_eq!(
            steps
                .iter()
                .filter(|s| matches!(s, Step::Hide(_, true)))
                .count(),
            2
        );
    }

    #[test]
    fn keeping_a_binding_again_puts_back_the_key_it_had_or_else_its_default() {
        let mut purged = Purged::default();
        let moved = b("key.drop", "q");
        let never_bound = b("zoom:out", "");
        purged.purge(&moved);
        purged.purge(&never_bound);
        // After the purge both are unbound.
        let (moved, never_bound) = (b("key.drop", ""), b("zoom:out", ""));
        assert_eq!(
            purged.keep(&moved),
            [
                Step::Rebind("key.drop".into(), "key.keyboard.q".into(), 0),
                Step::Hide("key.drop".into(), false)
            ]
        );
        assert_eq!(
            purged.keep(&never_bound)[0],
            Step::Rebind("zoom:out".into(), "key.keyboard.x".into(), 0)
        );
        assert!(purged.0.is_empty());
    }

    #[test]
    fn enforcing_unbinds_a_purged_binding_the_player_rebound() {
        let purged = Purged::parse("key.drop\nzoom:in");
        let bindings = [b("key.drop", "q"), b("zoom:in", ""), b("key.jump", "space")];
        assert_eq!(
            purged.enforce(&bindings),
            [Step::Rebind("key.drop".into(), UNBOUND.into(), 0)]
        );
    }
}
