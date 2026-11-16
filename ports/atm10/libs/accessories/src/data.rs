//! Slot types and which entities get them, from the JSON mods ship.

use std::collections::BTreeMap;

use serde::Deserialize;

/// `data/<ns>/curios/slots/<name>.json`.
#[derive(Clone, Debug, PartialEq)]
pub struct SlotType {
    /// The file's name: `ring`.
    pub id: String,
    /// How many slots, before an entity rule changes it. Default 1.
    pub size: usize,
    /// Position in the screen's column; lower comes first. Default 1000.
    pub order: i32,
    /// Sprite for an empty slot, `curios:slot/empty_ring_slot` style.
    pub icon: Option<String>,
    /// Whether the slot has a cosmetic row beside it.
    pub add_cosmetic: bool,
}

#[derive(Deserialize)]
struct RawSlot {
    #[serde(default = "one")]
    size: usize,
    #[serde(default = "thousand")]
    order: i32,
    icon: Option<String>,
    #[serde(default)]
    add_cosmetic: bool,
}

fn one() -> usize {
    1
}
fn thousand() -> i32 {
    1000
}

/// `data/<ns>/curios/entities/<name>.json`: which entity types or tags get which slots.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct EntityRule {
    /// Entity type ids; `#tag` for a tag. `minecraft:player` is the usual entry.
    pub entities: Vec<String>,
    /// Slot ids to give them.
    pub slots: Vec<String>,
}

/// Every slot type and entity rule loaded.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlotTypes {
    pub types: BTreeMap<String, SlotType>,
    pub rules: Vec<EntityRule>,
}

impl SlotTypes {
    /// Adds the slot file `id` (its name without `.json`). A later file of the same name
    /// replaces the earlier, as a datapack's does. Unreadable files are skipped and named.
    pub fn add_slot(&mut self, id: &str, json: &[u8]) -> Result<(), serde_json::Error> {
        let raw: RawSlot = serde_json::from_slice(json)?;
        self.types.insert(
            id.to_owned(),
            SlotType { id: id.to_owned(), size: raw.size, order: raw.order, icon: raw.icon, add_cosmetic: raw.add_cosmetic },
        );
        Ok(())
    }

    pub fn add_entities(&mut self, json: &[u8]) -> Result<(), serde_json::Error> {
        self.rules.push(serde_json::from_slice(json)?);
        Ok(())
    }

    /// The slot types an entity of type `entity_type` carries, by order then name. Tag entries
    /// (`#...`) are not matched here: pass the type's tags in `tags`.
    pub fn for_entity(&self, entity_type: &str, tags: &[&str]) -> Vec<&SlotType> {
        let mut out: Vec<&SlotType> = self
            .rules
            .iter()
            .filter(|r| r.entities.iter().any(|e| e == entity_type || e.strip_prefix('#').is_some_and(|t| tags.contains(&t))))
            .flat_map(|r| r.slots.iter())
            .filter_map(|s| self.types.get(s))
            .collect();
        out.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
        out.dedup_by(|a, b| a.id == b.id);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_default_and_order() {
        let mut t = SlotTypes::default();
        t.add_slot("ring", br#"{"size": 2, "order": 20, "add_cosmetic": true}"#).unwrap();
        t.add_slot("charm", b"{}").unwrap();
        t.add_slot("belt", br#"{"order": 5}"#).unwrap();
        t.add_entities(br#"{"entities": ["minecraft:player"], "slots": ["ring", "charm", "belt"]}"#).unwrap();
        t.add_entities(br#"{"entities": ["minecraft:player"], "slots": ["ring"]}"#).unwrap();
        let ids: Vec<&str> = t.for_entity("minecraft:player", &[]).iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["belt", "ring", "charm"]);
        assert_eq!(t.types["charm"].size, 1);
        assert_eq!(t.types["charm"].order, 1000);
        assert!(t.for_entity("minecraft:cow", &[]).is_empty());
    }

    #[test]
    fn an_entity_tag_matches() {
        let mut t = SlotTypes::default();
        t.add_slot("back", b"{}").unwrap();
        t.add_entities(br##"{"entities": ["#minecraft:skeletons"], "slots": ["back"]}"##).unwrap();
        assert_eq!(t.for_entity("minecraft:stray", &["minecraft:skeletons"]).len(), 1);
    }
}
