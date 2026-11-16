//! What each food store takes, apart from the game.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Store {
    Pantry,
    Fridge,
}

impl Store {
    pub const ALL: [Store; 2] = [Store::Pantry, Store::Fridge];

    pub fn name(self) -> &'static str {
        match self {
            Store::Pantry => "pantry",
            Store::Fridge => "fridge",
        }
    }

    pub fn of_block(block: &str) -> Option<Store> {
        let name = block.strip_prefix("storagedelight:")?;
        Store::ALL.into_iter().find(|s| s.name() == name)
    }

    /// 27 slots for the pantry, 18 for the fridge.
    pub fn slots(self) -> usize {
        match self {
            Store::Pantry => 27,
            Store::Fridge => 18,
        }
    }

    pub fn menu(self) -> &'static str {
        match self {
            Store::Pantry => "minecraft:generic_9x3",
            Store::Fridge => "minecraft:generic_9x2",
        }
    }

    /// Whether `item` may be stored: `is_food` is whether it carries the food component. The
    /// fridge also keeps milk and bottled drinks.
    pub fn accepts(self, item: &str, is_food: bool) -> bool {
        is_food
            || (self == Store::Fridge
                && matches!(item, "minecraft:milk_bucket" | "minecraft:potion" | "minecraft:honey_bottle"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pantry_takes_food_only_and_the_fridge_also_drinks() {
        assert!(Store::Pantry.accepts("minecraft:bread", true));
        assert!(!Store::Pantry.accepts("minecraft:stone", false));
        assert!(!Store::Pantry.accepts("minecraft:milk_bucket", false));
        assert!(Store::Fridge.accepts("minecraft:milk_bucket", false));
        assert!(!Store::Fridge.accepts("minecraft:stick", false));
    }

    #[test]
    fn blocks_name_their_store() {
        assert_eq!(Store::of_block("storagedelight:fridge"), Some(Store::Fridge));
        assert_eq!(Store::of_block("storagedelight:crate"), None);
        assert_eq!(Store::of_block("minecraft:pantry"), None);
    }
}
