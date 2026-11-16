//! KeybindsPurger's choices, apart from the game: which bindings to purge, saved as a list, and
//! whether a key press must be dropped because every binding on that key is purged.

use std::collections::BTreeSet;

/// One binding: its id (`key.jump`) and its key as Controls shows it, `None` when unbound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub id: String,
    pub key: Option<String>,
}

/// The purged bindings' ids.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Purged(pub BTreeSet<String>);

impl Purged {
    /// Reads the saved list: one id per line; blank lines and `#` comments are skipped.
    pub fn parse(text: &str) -> Self {
        Purged(
            text.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(str::to_owned)
                .collect(),
        )
    }

    /// The list as it is saved.
    pub fn write(&self) -> String {
        let mut out = String::from("# KeybindsPurger: one purged key binding per line\n");
        for id in &self.0 {
            out.push_str(id);
            out.push('\n');
        }
        out
    }

    pub fn contains(&self, id: &str) -> bool {
        self.0.contains(id)
    }

    /// Purges `id`, or keeps it again.
    pub fn toggle(&mut self, id: &str) {
        if !self.0.remove(id) {
            self.0.insert(id.to_owned());
        }
    }

    /// Purges every binding that shares its key with an earlier one, so each key keeps the first
    /// binding in Controls' order (vanilla's before mods'). Bindings already purged don't count
    /// as holding their key. Returns how many were added.
    pub fn purge_conflicts(&mut self, bindings: &[Binding]) -> usize {
        let mut held: BTreeSet<&str> = BTreeSet::new();
        let mut added = 0;
        for b in bindings {
            let Some(key) = &b.key else { continue };
            if self.contains(&b.id) {
                continue;
            }
            if !held.insert(key.as_str()) {
                self.0.insert(b.id.clone());
                added += 1;
            }
        }
        added
    }

    /// The purged bindings still bound to a key: what an unbind would clear.
    pub fn to_unbind<'a>(&self, bindings: &'a [Binding]) -> Vec<&'a str> {
        bindings
            .iter()
            .filter(|b| b.key.is_some() && self.contains(&b.id))
            .map(|b| b.id.as_str())
            .collect()
    }

    /// Whether a press reaching `mapping` must be dropped: it is purged and so is every other
    /// binding on its key (dropping a press drops it for all of them).
    pub fn drops(&self, mapping: &str, bindings: &[Binding]) -> bool {
        if !self.contains(mapping) {
            return false;
        }
        let Some(key) = bindings
            .iter()
            .find(|b| b.id == mapping)
            .and_then(|b| b.key.as_ref())
        else {
            return true;
        };
        bindings
            .iter()
            .filter(|b| b.key.as_ref() == Some(key))
            .all(|b| self.contains(&b.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(id: &str, key: Option<&str>) -> Binding {
        Binding {
            id: id.into(),
            key: key.map(str::to_owned),
        }
    }

    #[test]
    fn the_saved_list_reads_back() {
        let mut purged = Purged::default();
        purged.toggle("key.advancements");
        purged.toggle("key.socialInteractions");
        purged.toggle("key.advancements");
        assert_eq!(Purged::parse(&purged.write()), purged);
        assert_eq!(
            purged.0.iter().collect::<Vec<_>>(),
            ["key.socialInteractions"]
        );
        let edited = Purged::parse("# mine\n\n  key.chat  \nkey.drop\n");
        assert!(edited.contains("key.chat") && edited.contains("key.drop"));
        assert_eq!(edited.0.len(), 2);
    }

    #[test]
    fn purging_conflicts_keeps_the_first_binding_of_each_key() {
        let bindings = [
            b("key.drop", Some("Q")),
            b("key.jump", Some("Space")),
            b("mymod.dash", Some("Space")),
            b("othermod.fly", Some("Space")),
            b("mymod.menu", Some("Q")),
            b("key.smoothCamera", None),
        ];
        let mut purged = Purged::default();
        assert_eq!(purged.purge_conflicts(&bindings), 3);
        assert!(!purged.contains("key.jump") && !purged.contains("key.drop"));
        assert!(purged.contains("mymod.dash") && purged.contains("othermod.fly"));
        assert!(purged.contains("mymod.menu"));
        // A second pass finds nothing more.
        assert_eq!(purged.purge_conflicts(&bindings), 0);
    }

    #[test]
    fn a_purged_first_binding_lets_the_next_one_keep_the_key() {
        let bindings = [b("key.jump", Some("Space")), b("mymod.dash", Some("Space"))];
        let mut purged = Purged::parse("key.jump");
        purged.purge_conflicts(&bindings);
        assert!(!purged.contains("mymod.dash"));
    }

    #[test]
    fn a_press_drops_only_when_every_binding_on_its_key_is_purged() {
        let bindings = [
            b("key.advancements", Some("L")),
            b("key.jump", Some("Space")),
            b("key.swapOffhand", Some("Space")),
        ];
        let purged = Purged::parse("key.advancements\nkey.swapOffhand");
        assert!(purged.drops("key.advancements", &bindings));
        assert!(
            !purged.drops("key.swapOffhand", &bindings),
            "jump still wants Space"
        );
        assert!(!purged.drops("key.jump", &bindings));
        assert_eq!(
            purged.to_unbind(&bindings),
            ["key.advancements", "key.swapOffhand"]
        );
    }
}
