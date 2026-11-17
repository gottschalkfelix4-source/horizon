//! What each shelf takes, apart from the game.

pub const WOODS: [&str; 6] = ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shelf {
    Bookcase,
    PotionShelf,
}

impl Shelf {
    pub const ALL: [Shelf; 2] = [Shelf::Bookcase, Shelf::PotionShelf];

    pub fn name(self) -> &'static str {
        match self {
            Shelf::Bookcase => "bookcase",
            Shelf::PotionShelf => "potion_shelf",
        }
    }

    /// Slots: the bookcase has two rows of nine, the potion shelf one.
    pub fn slots(self) -> usize {
        match self {
            Shelf::Bookcase => 18,
            Shelf::PotionShelf => 9,
        }
    }

    /// The vanilla menu that shows the slots.
    pub fn menu(self) -> &'static str {
        match self {
            Shelf::Bookcase => "minecraft:generic_9x2",
            Shelf::PotionShelf => "minecraft:generic_9x1",
        }
    }

    /// What goes on this shelf.
    pub fn accepts(self, item: &str) -> bool {
        match self {
            Shelf::Bookcase => matches!(
                item,
                "minecraft:book" | "minecraft:writable_book" | "minecraft:written_book" | "minecraft:enchanted_book" | "minecraft:knowledge_book"
            ),
            Shelf::PotionShelf => matches!(
                item,
                "minecraft:potion"
                    | "minecraft:splash_potion"
                    | "minecraft:lingering_potion"
                    | "minecraft:glass_bottle"
                    | "minecraft:experience_bottle"
                    | "minecraft:honey_bottle"
            ),
        }
    }
}

/// The block id `bibliocraft:<wood>_<shelf>`.
pub fn block_id(wood: &str, shelf: Shelf) -> String {
    format!("bibliocraft:{wood}_{}", shelf.name())
}

/// Which shelf a block id is.
pub fn shelf_of(block: &str) -> Option<Shelf> {
    let name = block.strip_prefix("bibliocraft:")?;
    Shelf::ALL.into_iter().find(|s| {
        WOODS.iter().any(|w| name == format!("{w}_{}", s.name()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bookcase_takes_books_and_not_potions_and_the_potion_shelf_the_other_way_round() {
        assert!(Shelf::Bookcase.accepts("minecraft:enchanted_book"));
        assert!(!Shelf::Bookcase.accepts("minecraft:potion"));
        assert!(Shelf::PotionShelf.accepts("minecraft:lingering_potion"));
        assert!(!Shelf::PotionShelf.accepts("minecraft:book"));
        assert!(!Shelf::Bookcase.accepts("minecraft:paper"));
    }

    #[test]
    fn block_ids_name_a_wood_and_a_shelf() {
        assert_eq!(shelf_of(&block_id("dark_oak", Shelf::PotionShelf)), Some(Shelf::PotionShelf));
        assert_eq!(shelf_of("bibliocraft:oak_bookcase"), Some(Shelf::Bookcase));
        assert_eq!(shelf_of("bibliocraft:cherry_bookcase"), None);
        assert_eq!(shelf_of("minecraft:oak_bookcase"), None);
    }
}
