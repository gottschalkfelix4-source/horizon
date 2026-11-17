//! Crop tiers and farmland tiers of Mystical Agradditions, apart from the game.
//!
//! Mystical Agriculture's farmland runs from tier 1 (inferium) to 5 (supremium); this mod adds
//! tier 6, insanium farmland, and six awakened crops that need it.

/// The awakened crops (path of `<name>_crop`, `<name>_seeds`, `<name>_essence`), all tier 6.
pub const CROPS: [&str; 6] = [
    "nether_star",
    "dragon_egg",
    "awakened_draconium",
    "gaia_spirit",
    "neutronium",
    "nitro_crystal",
];

/// The tier of the awakened crops.
pub const CROP_TIER: u32 = 6;

/// Mystical Agriculture's farmlands in tier order (`mysticalagriculture:<name>_farmland`).
const MA_FARMLANDS: [&str; 5] = ["inferium", "prudentium", "tertium", "imperium", "supremium"];

pub fn crop_tier(block: &str) -> Option<u32> {
    let name = block
        .strip_prefix("mysticalagradditions:")?
        .strip_suffix("_crop")?;
    CROPS.contains(&name).then_some(CROP_TIER)
}

/// Vanilla farmland is below every tier (0), Mystical Agriculture's is its number, insanium
/// farmland is 6; anything else is no farmland.
pub fn farmland_tier(block: &str) -> Option<u32> {
    match block {
        "minecraft:farmland" => return Some(0),
        "mysticalagradditions:insanium_farmland" => return Some(6),
        _ => {}
    }
    let name = block
        .strip_prefix("mysticalagriculture:")?
        .strip_suffix("_farmland")?;
    MA_FARMLANDS
        .iter()
        .position(|n| *n == name)
        .map(|i| i as u32 + 1)
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
    fn an_awakened_crop_grows_only_on_insanium_farmland() {
        assert!(can_grow(
            "mysticalagradditions:insanium_farmland",
            "mysticalagradditions:neutronium_crop"
        ));
        for lower in [
            "minecraft:farmland",
            "mysticalagriculture:inferium_farmland",
            "mysticalagriculture:supremium_farmland",
        ] {
            assert!(
                !can_grow(lower, "mysticalagradditions:nether_star_crop"),
                "{lower}"
            );
        }
    }

    #[test]
    fn what_is_no_awakened_crop_or_no_farmland_never_grows() {
        assert!(!can_grow(
            "mysticalagradditions:insanium_farmland",
            "mysticalagriculture:earth_crop"
        ));
        assert!(!can_grow(
            "minecraft:dirt",
            "mysticalagradditions:dragon_egg_crop"
        ));
        assert_eq!(crop_tier("mysticalagradditions:gaia_spirit_seeds"), None);
        assert_eq!(
            farmland_tier("mysticalagriculture:imperium_farmland"),
            Some(4)
        );
    }
}
