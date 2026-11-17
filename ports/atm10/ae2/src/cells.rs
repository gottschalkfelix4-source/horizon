//! Storage cells: the numbers and the pure bookkeeping of what a cell holds. A cell's contents are
//! saved in the stack's `minecraft:custom_data` under `ae2_cell` as `{<item id>: <count as long>}`,
//! so a cell keeps them wherever it goes.
//!
//! Capacity as AE2 plays it: a cell has `bytes`; every stored item type costs `bytes / 128` of them
//! (up to 63 types), every 8 items cost one byte.
use mod_api::{Compound, ItemStack, Tag};

pub const MAX_TYPES: usize = 63;
pub const ITEMS_PER_BYTE: u64 = 8;
pub const DRIVE_CELLS: usize = 10;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellTier {
    K1,
    K4,
    K16,
    K64,
    K256,
}

impl CellTier {
    pub const ALL: [CellTier; 5] = [CellTier::K1, CellTier::K4, CellTier::K16, CellTier::K64, CellTier::K256];

    pub fn name(self) -> &'static str {
        match self {
            CellTier::K1 => "1k",
            CellTier::K4 => "4k",
            CellTier::K16 => "16k",
            CellTier::K64 => "64k",
            CellTier::K256 => "256k",
        }
    }

    pub fn bytes(self) -> u64 {
        1024
            * match self {
                CellTier::K1 => 1,
                CellTier::K4 => 4,
                CellTier::K16 => 16,
                CellTier::K64 => 64,
                CellTier::K256 => 256,
            }
    }

    /// Bytes each stored item type costs.
    pub fn type_bytes(self) -> u64 {
        self.bytes() / 128
    }

    pub fn id(self) -> String {
        format!("ae2:item_storage_cell_{}", self.name())
    }
}

/// `ae2:item_storage_cell_<tier>` as a tier.
pub fn parse_cell(id: &str) -> Option<CellTier> {
    let t = id.strip_prefix("ae2:item_storage_cell_")?;
    CellTier::ALL.into_iter().find(|c| c.name() == t)
}

/// What a cell holds.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Store {
    pub items: Vec<(String, u64)>,
}

impl Store {
    pub fn types(&self) -> usize {
        self.items.len()
    }

    pub fn total(&self) -> u64 {
        self.items.iter().map(|(_, n)| n).sum()
    }

    pub fn count(&self, id: &str) -> u64 {
        self.items.iter().find(|(i, _)| i == id).map_or(0, |(_, n)| *n)
    }

    /// Bytes in use: the type overhead and one per eight items, rounded up.
    pub fn used_bytes(&self, tier: CellTier) -> u64 {
        self.types() as u64 * tier.type_bytes() + self.total().div_ceil(ITEMS_PER_BYTE)
    }

    /// How many of `id` still fit.
    pub fn room_for(&self, tier: CellTier, id: &str) -> u64 {
        let new = self.count(id) == 0;
        let types = self.types() + usize::from(new);
        if types > MAX_TYPES {
            return 0;
        }
        let bytes = tier.bytes().saturating_sub(types as u64 * tier.type_bytes());
        (bytes * ITEMS_PER_BYTE).saturating_sub(self.total())
    }

    /// Puts in up to `n`; how many went in.
    pub fn insert(&mut self, tier: CellTier, id: &str, n: u64) -> u64 {
        let take = n.min(self.room_for(tier, id));
        if take == 0 {
            return 0;
        }
        match self.items.iter_mut().find(|(i, _)| i == id) {
            Some((_, c)) => *c += take,
            None => self.items.push((id.to_string(), take)),
        }
        take
    }

    /// Takes out up to `n`; how many came out. An emptied type is forgotten.
    pub fn extract(&mut self, id: &str, n: u64) -> u64 {
        let Some(pos) = self.items.iter().position(|(i, _)| i == id) else { return 0 };
        let take = n.min(self.items[pos].1);
        self.items[pos].1 -= take;
        if self.items[pos].1 == 0 {
            self.items.remove(pos);
        }
        take
    }

    /// The contents a cell stack carries.
    pub fn of_stack(stack: &ItemStack) -> Store {
        let Some(Tag::Compound(data)) = stack.components().and_then(|c| c.as_nbt().get(&"minecraft:custom_data".into())) else {
            return Store::default();
        };
        let Some(Tag::Compound(cell)) = data.get(&"ae2_cell".into()) else { return Store::default() };
        let mut items: Vec<(String, u64)> = cell
            .iter()
            .filter_map(|(k, v)| match v {
                Tag::Long(n) if *n > 0 => Some((k.to_string_lossy(), *n as u64)),
                _ => None,
            })
            .collect();
        items.sort();
        Store { items }
    }

    /// Writes the contents into the cell stack.
    pub fn write_to(&self, stack: &mut ItemStack) {
        let mut cell = Compound::new();
        for (id, n) in &self.items {
            cell.insert(id.as_str().into(), Tag::Long(*n as i64));
        }
        let mut data = Compound::new();
        data.insert("ae2_cell".into(), Tag::Compound(cell));
        let mut patch = Compound::new();
        patch.insert("minecraft:custom_data".into(), Tag::Compound(data));
        let _ = stack.apply_components(&patch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_1k_cell_holds_8128_of_one_type() {
        // 1024 bytes - 8 for the type = 1016 bytes * 8 items
        let mut s = Store::default();
        assert_eq!(s.insert(CellTier::K1, "minecraft:cobblestone", 100_000), 8128);
        assert_eq!(s.used_bytes(CellTier::K1), 1024);
        assert_eq!(s.insert(CellTier::K1, "minecraft:dirt", 1), 0);
    }

    #[test]
    fn a_second_type_costs_its_overhead_in_room() {
        let mut s = Store::default();
        s.insert(CellTier::K1, "minecraft:cobblestone", 800);
        // two types: (1024 - 16) * 8 = 8064 items in all, 800 already in
        assert_eq!(s.room_for(CellTier::K1, "minecraft:dirt"), 7264);
        assert_eq!(s.insert(CellTier::K1, "minecraft:dirt", 10_000), 7264);
    }

    #[test]
    fn sixty_three_types_at_most() {
        let mut s = Store::default();
        for i in 0..70 {
            s.insert(CellTier::K256, &format!("minecraft:item_{i}"), 1);
        }
        assert_eq!(s.types(), 63);
    }

    #[test]
    fn extracting_the_last_forgets_the_type() {
        let mut s = Store::default();
        s.insert(CellTier::K4, "minecraft:sand", 5);
        assert_eq!(s.extract("minecraft:sand", 9), 5);
        assert_eq!(s.types(), 0);
        assert_eq!(s.extract("minecraft:sand", 1), 0);
    }

    #[test]
    fn cell_ids() {
        assert_eq!(parse_cell("ae2:item_storage_cell_64k"), Some(CellTier::K64));
        assert_eq!(parse_cell("ae2:item_storage_cell_2k"), None);
        assert_eq!(CellTier::K16.id(), "ae2:item_storage_cell_16k");
    }
}
