//! The Filter Module (`rftoolsbase:filter_module`): a list of ghost items and the rules a stack is
//! compared to them by, shared by the RFTools machines and XNet's connectors.
//!
//! Options, as the module's screen shows them: blacklist (invert), match damage, match
//! components (NBT before 1.20.5), match tags (the old ore dictionary), match mod (any item of
//! the same namespace). The configuration is saved in the item's `minecraft:custom_data`.
use mod_api::{Compound, Tag};

/// What the filter needs to know of a stack, read from the game by the caller.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemFacts {
    /// `minecraft:iron_ingot`.
    pub id: String,
    /// Damage taken (0 for an undamaged or undamageable item).
    pub damage: u32,
    /// The stack's component patch, compared whole when components are matched.
    pub components: Compound,
    /// The item tags it is in, without `#`.
    pub tags: Vec<String>,
}

impl ItemFacts {
    pub fn named(id: &str) -> Self {
        Self { id: id.into(), ..Default::default() }
    }

    fn namespace(&self) -> &str {
        self.id.split_once(':').map_or("minecraft", |(ns, _)| ns)
    }
}

/// A filter module's settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filter {
    pub blacklist: bool,
    pub damage: bool,
    pub components: bool,
    pub tags: bool,
    pub mod_match: bool,
    pub items: Vec<ItemFacts>,
}

impl Filter {
    /// Whether one ghost item stands for `stack` under these rules.
    fn same(&self, ghost: &ItemFacts, stack: &ItemFacts) -> bool {
        if self.mod_match {
            return ghost.namespace() == stack.namespace();
        }
        if self.tags && ghost.id != stack.id {
            // Tag mode: any shared tag counts, so copper ingots of every mod pass one ingot.
            return ghost.tags.iter().any(|t| stack.tags.contains(t));
        }
        if ghost.id != stack.id {
            return false;
        }
        if self.damage && ghost.damage != stack.damage {
            return false;
        }
        if self.components && ghost.components != stack.components {
            return false;
        }
        true
    }

    /// Whether the filter lets `stack` through. An empty whitelist lets nothing through and an
    /// empty blacklist lets everything through.
    pub fn accepts(&self, stack: &ItemFacts) -> bool {
        let listed = self.items.iter().any(|g| self.same(g, stack));
        listed != self.blacklist
    }

    /// The settings as saved in the module's `custom_data`.
    pub fn save(&self) -> Compound {
        let mut nbt = Compound::new();
        for (key, on) in [
            ("blacklist", self.blacklist),
            ("damage", self.damage),
            ("nbt", self.components),
            ("tags", self.tags),
            ("mod", self.mod_match),
        ] {
            nbt.insert(key.into(), Tag::Byte(i8::from(on)));
        }
        let list = self
            .items
            .iter()
            .map(|i| {
                let mut c = Compound::new();
                c.insert("id".into(), Tag::String(i.id.as_str().into()));
                c.insert("damage".into(), Tag::Int(i.damage as i32));
                c.insert("components".into(), Tag::Compound(i.components.clone()));
                c.insert(
                    "tags".into(),
                    Tag::List(i.tags.iter().map(|t| Tag::String(t.as_str().into())).collect()),
                );
                Tag::Compound(c)
            })
            .collect();
        nbt.insert("items".into(), Tag::List(list));
        nbt
    }

    /// `save`'s reader; unknown or missing keys read as off and empty.
    pub fn load(nbt: &Compound) -> Self {
        let flag = |k: &str| matches!(nbt.get(&k.into()), Some(Tag::Byte(1)));
        let items = match nbt.get(&"items".into()) {
            Some(Tag::List(list)) => list
                .iter()
                .filter_map(|t| match t {
                    Tag::Compound(c) => Some(ItemFacts {
                        id: match c.get(&"id".into()) {
                            Some(Tag::String(s)) => s.to_string_lossy(),
                            _ => return None,
                        },
                        damage: match c.get(&"damage".into()) {
                            Some(Tag::Int(d)) => (*d).max(0) as u32,
                            _ => 0,
                        },
                        components: match c.get(&"components".into()) {
                            Some(Tag::Compound(p)) => p.clone(),
                            _ => Compound::new(),
                        },
                        tags: match c.get(&"tags".into()) {
                            Some(Tag::List(l)) => l
                                .iter()
                                .filter_map(|t| match t {
                                    Tag::String(s) => Some(s.to_string_lossy()),
                                    _ => None,
                                })
                                .collect(),
                            _ => Vec::new(),
                        },
                    }),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        Self {
            blacklist: flag("blacklist"),
            damage: flag("damage"),
            components: flag("nbt"),
            tags: flag("tags"),
            mod_match: flag("mod"),
            items,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ingot(id: &str, tag: &str) -> ItemFacts {
        ItemFacts { id: id.into(), tags: vec![tag.into()], ..Default::default() }
    }

    #[test]
    fn an_empty_whitelist_refuses_and_an_empty_blacklist_accepts() {
        let stone = ItemFacts::named("minecraft:stone");
        assert!(!Filter::default().accepts(&stone));
        assert!(Filter { blacklist: true, ..Default::default() }.accepts(&stone));
    }

    #[test]
    fn damage_only_matters_when_asked() {
        let new = ItemFacts::named("minecraft:iron_pickaxe");
        let worn = ItemFacts { damage: 40, ..new.clone() };
        let loose = Filter { items: vec![new.clone()], ..Default::default() };
        assert!(loose.accepts(&worn));
        let strict = Filter { damage: true, ..loose };
        assert!(!strict.accepts(&worn));
        assert!(strict.accepts(&new));
    }

    #[test]
    fn tag_mode_passes_another_mods_item_in_a_shared_tag() {
        let filter = Filter {
            tags: true,
            items: vec![ingot("minecraft:copper_ingot", "c:ingots/copper")],
            ..Default::default()
        };
        assert!(filter.accepts(&ingot("othermod:copper_ingot", "c:ingots/copper")));
        assert!(!filter.accepts(&ingot("othermod:tin_ingot", "c:ingots/tin")));
    }

    #[test]
    fn mod_mode_passes_any_item_of_the_namespace_and_blacklist_inverts_it() {
        let filter = Filter {
            mod_match: true,
            blacklist: true,
            items: vec![ItemFacts::named("rftoolsbase:machine_frame")],
            ..Default::default()
        };
        assert!(!filter.accepts(&ItemFacts::named("rftoolsbase:dimensionalshard")));
        assert!(filter.accepts(&ItemFacts::named("minecraft:dirt")));
    }

    #[test]
    fn components_are_compared_whole() {
        let mut named = Compound::new();
        named.insert("minecraft:custom_name".into(), Tag::String("Bob".into()));
        let plain = ItemFacts::named("minecraft:diamond_sword");
        let filter = Filter { components: true, items: vec![plain.clone()], ..Default::default() };
        assert!(filter.accepts(&plain));
        assert!(!filter.accepts(&ItemFacts { components: named, ..plain }));
    }

    #[test]
    fn settings_round_trip_through_custom_data() {
        let filter = Filter {
            blacklist: true,
            damage: false,
            components: true,
            tags: true,
            mod_match: false,
            items: vec![ingot("minecraft:gold_ingot", "c:ingots/gold"), ItemFacts { damage: 3, ..ItemFacts::named("minecraft:bow") }],
        };
        assert_eq!(Filter::load(&filter.save()), filter);
    }
}
