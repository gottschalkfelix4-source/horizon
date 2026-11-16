//! Crop tiers and farmland tiers, apart from the game.

/// The crops: path (`<name>_crop`, `<name>_seeds`, `<name>_essence`) and tier, 1 to 5.
pub const CROPS: [(&str, u32); 16] = [
    ("inferium", 1),
    ("prudentium", 2),
    ("tertium", 3),
    ("imperium", 4),
    ("supremium", 5),
    ("earth", 1),
    ("air", 1),
    ("water", 1),
    ("fire", 1),
    ("stone", 1),
    ("coal", 2),
    ("iron", 2),
    ("gold", 3),
    ("redstone", 3),
    ("diamond", 4),
    ("emerald", 4),
];

/// The farmlands, in tier order: `<name>_farmland` is tier 1 to 5.
pub const FARMLANDS: [&str; 5] = ["inferium", "prudentium", "tertium", "imperium", "supremium"];

pub fn crop_tier(block: &str) -> Option<u32> {
    let name = block.strip_prefix("mysticalagriculture:")?.strip_suffix("_crop")?;
    CROPS.iter().find(|(n, _)| *n == name).map(|(_, t)| *t)
}

/// The tier of a farmland block: vanilla farmland is below every tier (0), a mod farmland is its
/// number, anything else has none.
pub fn farmland_tier(block: &str) -> Option<u32> {
    if block == "minecraft:farmland" {
        return Some(0);
    }
    let name = block.strip_prefix("mysticalagriculture:")?.strip_suffix("_farmland")?;
    FARMLANDS.iter().position(|n| *n == name).map(|i| i as u32 + 1)
}

/// A crop grows only on farmland of its tier or higher.
pub fn can_grow(farmland: &str, crop: &str) -> bool {
    match (farmland_tier(farmland), crop_tier(crop)) {
        (Some(f), Some(c)) => f >= c,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crop_needs_farmland_of_its_tier_or_better() {
        assert!(can_grow("mysticalagriculture:inferium_farmland", "mysticalagriculture:earth_crop"));
        assert!(can_grow("mysticalagriculture:imperium_farmland", "mysticalagriculture:diamond_crop"));
        assert!(can_grow("mysticalagriculture:supremium_farmland", "mysticalagriculture:diamond_crop"));
        assert!(!can_grow("mysticalagriculture:prudentium_farmland", "mysticalagriculture:diamond_crop"));
        assert!(!can_grow("minecraft:farmland", "mysticalagriculture:inferium_crop"));
    }

    #[test]
    fn blocks_that_are_not_a_crop_or_a_farmland_never_grow_one() {
        assert!(!can_grow("minecraft:dirt", "mysticalagriculture:earth_crop"));
        assert!(!can_grow("mysticalagriculture:tertium_farmland", "minecraft:wheat"));
        assert_eq!(crop_tier("mysticalagriculture:gold_crop"), Some(3));
        assert_eq!(crop_tier("mysticalagriculture:gold_seeds"), None);
    }
}
