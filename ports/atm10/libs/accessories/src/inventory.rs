//! The worn items of one entity: per slot type, a row of stacks and a cosmetic row.

use std::collections::BTreeMap;

use mod_api::{Compound, Tag};

use crate::{data::SlotTypes, EquipError};

/// One slot type's stacks, as item NBT.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub stacks: Vec<Option<Compound>>,
    pub cosmetic: Vec<Option<Compound>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inventory {
    rows: BTreeMap<String, Row>,
}

impl Inventory {
    /// Sizes the rows to `types` for an entity of `entity_type`: new slot types appear, a row
    /// that grows is padded, and one that shrinks keeps its stacks (they stay worn but out of
    /// reach until the size returns, so nothing is lost when a mod is removed and added back).
    pub fn fit(&mut self, types: &SlotTypes, entity_type: &str) {
        for t in types.for_entity(entity_type, &[]) {
            let row = self.rows.entry(t.id.clone()).or_default();
            if row.stacks.len() < t.size {
                row.stacks.resize(t.size, None);
            }
            if t.add_cosmetic && row.cosmetic.len() < t.size {
                row.cosmetic.resize(t.size, None);
            }
        }
    }

    pub fn put(&mut self, slot: &str, index: usize, cosmetic: bool, stack: Compound) -> Result<(), EquipError> {
        let row = self.rows.get_mut(slot).ok_or(EquipError::NoSlot)?;
        let cell = if cosmetic { row.cosmetic.get_mut(index) } else { row.stacks.get_mut(index) }.ok_or(EquipError::NoSlot)?;
        if cell.is_some() {
            return Err(EquipError::Occupied);
        }
        *cell = Some(stack);
        Ok(())
    }

    pub fn take(&mut self, slot: &str, index: usize, cosmetic: bool) -> Option<Compound> {
        let row = self.rows.get_mut(slot)?;
        if cosmetic { row.cosmetic.get_mut(index) } else { row.stacks.get_mut(index) }?.take()
    }

    /// Every stack, by slot type name and index.
    pub fn items(&self, cosmetic: bool) -> impl Iterator<Item = (&str, usize, &Compound)> {
        self.rows.iter().flat_map(move |(name, row)| {
            let list = if cosmetic { &row.cosmetic } else { &row.stacks };
            list.iter().enumerate().filter_map(move |(i, s)| s.as_ref().map(|s| (name.as_str(), i, s)))
        })
    }

    pub fn row(&self, slot: &str) -> Option<&Row> {
        self.rows.get(slot)
    }

    /// `{rows: [{slot, stacks: [{Slot, ...item}], cosmetic: [...]}]}` as Curios keeps a player's
    /// items: empty cells are left out, each stack carries its index.
    pub fn to_nbt(&self) -> Compound {
        let list = |cells: &[Option<Compound>]| {
            Tag::List(
                cells
                    .iter()
                    .enumerate()
                    .filter_map(|(i, c)| {
                        let mut c = c.clone()?;
                        c.insert("Slot".into(), Tag::Int(i as i32));
                        Some(Tag::Compound(c))
                    })
                    .collect(),
            )
        };
        let rows = self
            .rows
            .iter()
            .map(|(name, row)| {
                let mut c = Compound::new();
                c.insert("slot".into(), Tag::String(name.as_str().into()));
                c.insert("size".into(), Tag::Int(row.stacks.len() as i32));
                c.insert("stacks".into(), list(&row.stacks));
                c.insert("cosmetic_size".into(), Tag::Int(row.cosmetic.len() as i32));
                c.insert("cosmetic".into(), list(&row.cosmetic));
                Tag::Compound(c)
            })
            .collect();
        [("rows".into(), Tag::List(rows))].into()
    }

    pub fn from_nbt(data: &Compound) -> Option<Self> {
        let Some(Tag::List(rows)) = data.get(&"rows".into()) else {
            return None;
        };
        let mut inv = Inventory::default();
        for row in rows {
            let Tag::Compound(row) = row else { return None };
            let Some(Tag::String(name)) = row.get(&"slot".into()) else { return None };
            let size = match row.get(&"size".into()) {
                Some(Tag::Int(n)) => usize::try_from(*n).ok()?,
                _ => 0,
            };
            let read = |key: &str, mut cells: Vec<Option<Compound>>| -> Option<Vec<Option<Compound>>> {
                if let Some(Tag::List(list)) = row.get(&key.into()) {
                    for e in list {
                        let Tag::Compound(c) = e else { return None };
                        let Some(Tag::Int(i)) = c.get(&"Slot".into()) else { return None };
                        let i = usize::try_from(*i).ok()?;
                        if i >= cells.len() {
                            cells.resize(i + 1, None);
                        }
                        let mut c = c.clone();
                        c.remove(&"Slot".into());
                        cells[i] = Some(c);
                    }
                }
                Some(cells)
            };
            let stacks = read("stacks", vec![None; size])?;
            let cosmetic_size = match row.get(&"cosmetic_size".into()) {
                Some(Tag::Int(n)) => usize::try_from(*n).ok()?,
                _ => 0,
            };
            let cosmetic = read("cosmetic", vec![None; cosmetic_size])?;
            inv.rows.insert(name.to_string_lossy(), Row { stacks, cosmetic });
        }
        Some(inv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(id: &str) -> Compound {
        [("id".into(), Tag::String(id.into())), ("count".into(), Tag::Int(1))].into()
    }

    fn types() -> SlotTypes {
        let mut t = SlotTypes::default();
        t.add_slot("ring", br#"{"size": 2, "add_cosmetic": true}"#).unwrap();
        t.add_entities(br#"{"entities": ["minecraft:player"], "slots": ["ring"]}"#).unwrap();
        t
    }

    #[test]
    fn put_take_and_the_rules_of_a_cell() {
        let mut inv = Inventory::default();
        assert_eq!(inv.put("ring", 0, false, stack("a")), Err(EquipError::NoSlot));
        inv.fit(&types(), "minecraft:player");
        assert_eq!(inv.put("ring", 2, false, stack("a")), Err(EquipError::NoSlot));
        inv.put("ring", 1, false, stack("a")).unwrap();
        assert_eq!(inv.put("ring", 1, false, stack("b")), Err(EquipError::Occupied));
        inv.put("ring", 1, true, stack("skin")).unwrap();
        assert_eq!(inv.take("ring", 1, false), Some(stack("a")));
        assert_eq!(inv.take("ring", 1, false), None);
        assert_eq!(inv.items(true).count(), 1);
    }

    #[test]
    fn saved_form_round_trips_and_keeps_a_shrunk_rows_items() {
        let mut inv = Inventory::default();
        inv.fit(&types(), "minecraft:player");
        inv.put("ring", 1, false, stack("a")).unwrap();
        inv.put("ring", 0, true, stack("b")).unwrap();
        assert_eq!(Inventory::from_nbt(&inv.to_nbt()), Some(inv.clone()));
        // The type now has size 1: the stack at index 1 is kept.
        let mut smaller = SlotTypes::default();
        smaller.add_slot("ring", br#"{"size": 1}"#).unwrap();
        smaller.add_entities(br#"{"entities": ["minecraft:player"], "slots": ["ring"]}"#).unwrap();
        inv.fit(&smaller, "minecraft:player");
        assert_eq!(inv.items(false).count(), 1);
    }
}
