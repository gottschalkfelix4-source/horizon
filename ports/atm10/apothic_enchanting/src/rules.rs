//! Shelf statistics, apart from the game.

/// A shelf's contribution to an enchanting table, in Apothic Enchanting's terms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shelf {
    pub name: &'static str,
    /// Eterna each shelf adds.
    pub eterna: f32,
    /// The most eterna a table using this shelf can reach.
    pub max_eterna: f32,
}

pub const SHELVES: [Shelf; 4] = [
    Shelf { name: "hellshelf", eterna: 1.5, max_eterna: 22.5 },
    Shelf { name: "infused_hellshelf", eterna: 2.0, max_eterna: 45.0 },
    Shelf { name: "seashelf", eterna: 1.5, max_eterna: 22.5 },
    Shelf { name: "infused_seashelf", eterna: 2.0, max_eterna: 45.0 },
];

/// The shelf a block id is.
pub fn shelf_of(block: &str) -> Option<&'static Shelf> {
    let name = block.strip_prefix("apothic_enchanting:")?;
    SHELVES.iter().find(|s| s.name == name)
}

/// The table's eterna from the shelves around it: the sum of what each adds, held to the
/// smallest maximum of any kind of shelf present (one weak shelf caps the table).
pub fn eterna<'a>(shelves: impl IntoIterator<Item = &'a Shelf>) -> f32 {
    let (mut sum, mut cap, mut any) = (0.0, f32::MAX, false);
    for s in shelves {
        any = true;
        sum += s.eterna;
        cap = cap.min(s.max_eterna);
    }
    if any { sum.min(cap) } else { 0.0 }
}

/// Vanilla's ring of cells a bookshelf can sit in around a table: two out in a square ring, at the
/// table's height and one above (`EnchantingTableBlock.BOOKSHELF_OFFSETS`).
pub fn ring() -> Vec<[i32; 3]> {
    let mut v = Vec::new();
    for dx in -2i32..=2 {
        for dz in -2i32..=2 {
            if dx.abs() == 2 || dz.abs() == 2 {
                for dy in 0..=1 {
                    v.push([dx, dy, dz]);
                }
            }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eterna_adds_up_and_stops_at_the_weakest_shelfs_maximum() {
        let hell = shelf_of("apothic_enchanting:hellshelf").unwrap();
        let infused = shelf_of("apothic_enchanting:infused_hellshelf").unwrap();
        assert_eq!(eterna(std::iter::repeat(hell).take(10)), 15.0);
        assert_eq!(eterna(std::iter::repeat(hell).take(20)), 22.5);
        // twenty infused ones would make 40, but one plain hellshelf holds the table at 22.5
        let mixed = std::iter::repeat(infused).take(20).chain([hell]);
        assert_eq!(eterna(mixed), 22.5);
        assert_eq!(eterna(std::iter::repeat(infused).take(20)), 40.0);
    }

    #[test]
    fn a_table_with_no_shelves_has_no_eterna() {
        assert_eq!(eterna(std::iter::empty()), 0.0);
    }

    #[test]
    fn the_ring_has_the_32_cells_vanilla_checks() {
        // 16 positions around the table, at two heights
        assert_eq!(ring().len(), 32);
        assert!(ring().iter().all(|[x, _, z]| x.abs() == 2 || z.abs() == 2));
    }
}
