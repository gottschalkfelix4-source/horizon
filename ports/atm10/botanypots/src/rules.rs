//! Soils, crops and the growing clock, apart from the game.

/// A soil: the block item that goes in the pot's soil slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Soil {
    pub item: &'static str,
    pub categories: &'static [&'static str],
    /// Fraction of a crop's growth time this soil saves (0.1: 10 % faster).
    pub bonus: f32,
}

pub const SOILS: [Soil; 9] = [
    Soil { item: "minecraft:dirt", categories: &["dirt"], bonus: 0.0 },
    Soil { item: "minecraft:grass_block", categories: &["dirt"], bonus: 0.05 },
    Soil { item: "minecraft:podzol", categories: &["dirt"], bonus: 0.05 },
    Soil { item: "minecraft:farmland", categories: &["farmland", "dirt"], bonus: 0.1 },
    Soil { item: "minecraft:sand", categories: &["sand"], bonus: 0.0 },
    Soil { item: "minecraft:red_sand", categories: &["sand"], bonus: 0.0 },
    Soil { item: "minecraft:soul_sand", categories: &["soul_sand"], bonus: 0.0 },
    Soil { item: "minecraft:mycelium", categories: &["mycelium", "dirt"], bonus: 0.05 },
    Soil { item: "minecraft:end_stone", categories: &["end_stone"], bonus: 0.0 },
];

/// One thing a harvest gives: item, least, most, chance in percent.
pub type Drop = (&'static str, u32, u32, u32);

/// A crop: the seed item, the soil categories it grows on, its growth time and what it gives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crop {
    pub seed: &'static str,
    pub categories: &'static [&'static str],
    pub ticks: u32,
    pub drops: &'static [Drop],
}

pub const CROPS: [Crop; 11] = [
    Crop { seed: "minecraft:wheat_seeds", categories: &["dirt", "farmland"], ticks: 2400, drops: &[("minecraft:wheat", 1, 2, 100), ("minecraft:wheat_seeds", 1, 2, 80)] },
    Crop { seed: "minecraft:carrot", categories: &["dirt", "farmland"], ticks: 2400, drops: &[("minecraft:carrot", 1, 3, 100)] },
    Crop { seed: "minecraft:potato", categories: &["dirt", "farmland"], ticks: 2400, drops: &[("minecraft:potato", 1, 3, 100), ("minecraft:poisonous_potato", 1, 1, 2)] },
    Crop { seed: "minecraft:beetroot_seeds", categories: &["dirt", "farmland"], ticks: 2400, drops: &[("minecraft:beetroot", 1, 1, 100), ("minecraft:beetroot_seeds", 1, 2, 80)] },
    Crop { seed: "minecraft:melon_seeds", categories: &["dirt", "farmland"], ticks: 3000, drops: &[("minecraft:melon_slice", 3, 7, 100)] },
    Crop { seed: "minecraft:pumpkin_seeds", categories: &["dirt", "farmland"], ticks: 3000, drops: &[("minecraft:pumpkin", 1, 1, 100)] },
    Crop { seed: "minecraft:sugar_cane", categories: &["sand", "dirt"], ticks: 2400, drops: &[("minecraft:sugar_cane", 2, 3, 100)] },
    Crop { seed: "minecraft:cactus", categories: &["sand"], ticks: 2400, drops: &[("minecraft:cactus", 2, 3, 100)] },
    Crop { seed: "minecraft:nether_wart", categories: &["soul_sand"], ticks: 3000, drops: &[("minecraft:nether_wart", 2, 4, 100)] },
    Crop { seed: "minecraft:oak_sapling", categories: &["dirt"], ticks: 4800, drops: &[("minecraft:oak_log", 3, 6, 100), ("minecraft:stick", 1, 3, 50), ("minecraft:apple", 1, 1, 5), ("minecraft:oak_sapling", 1, 1, 30)] },
    Crop { seed: "minecraft:red_mushroom", categories: &["mycelium"], ticks: 2400, drops: &[("minecraft:red_mushroom", 1, 2, 100)] },
];

pub fn soil_of(item: &str) -> Option<&'static Soil> {
    SOILS.iter().find(|s| s.item == item)
}

pub fn crop_of(seed: &str) -> Option<&'static Crop> {
    CROPS.iter().find(|c| c.seed == seed)
}

/// Ticks a crop takes on a soil, or None when the soil is not one it grows on.
pub fn growth_ticks(soil: &str, seed: &str) -> Option<u32> {
    let (s, c) = (soil_of(soil)?, crop_of(seed)?);
    c.categories.iter().any(|cat| s.categories.contains(cat)).then(|| {
        ((c.ticks as f32 * (1.0 - s.bonus)).floor() as u32).max(1)
    })
}

/// One tick of the clock: the progress after it, and whether the crop is ripe. With no soil or
/// seed, or a pair that does not match, the progress falls to nothing.
pub fn tick(progress: u32, soil: Option<&str>, seed: Option<&str>) -> (u32, bool) {
    match soil.zip(seed).and_then(|(s, c)| growth_ticks(s, c)) {
        Some(need) => {
            let p = (progress + 1).min(need);
            (p, p >= need)
        }
        None => (0, false),
    }
}

/// What one harvest gives, with `rolls` a stream of numbers 0 to 99 (one per chance and one per
/// count, in the order of the crop's drops).
pub fn harvest(crop: &Crop, mut rolls: impl FnMut() -> u32) -> Vec<(&'static str, u32)> {
    let mut out = Vec::new();
    for (item, least, most, chance) in crop.drops {
        if rolls() >= *chance {
            continue;
        }
        let span = most - least + 1;
        out.push((*item, least + rolls() % span));
    }
    out
}

/// A roll from the game time and position, the same on every run.
pub fn roll_stream(time: i64, pos: [i32; 3]) -> impl FnMut() -> u32 {
    let mut h = (time as u64) ^ (pos[0] as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (pos[1] as i64 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (pos[2] as i64 as u64).wrapping_mul(0x1656_67B1_9E37_79F9);
    move || {
        h ^= h >> 12;
        h ^= h << 25;
        h ^= h >> 27;
        (h.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33) as u32 % 100
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_soil_saves_a_fraction_of_the_growth_time_and_a_wrong_soil_stops_growth() {
        assert_eq!(growth_ticks("minecraft:dirt", "minecraft:wheat_seeds"), Some(2400));
        assert_eq!(growth_ticks("minecraft:farmland", "minecraft:wheat_seeds"), Some(2160));
        assert_eq!(growth_ticks("minecraft:sand", "minecraft:wheat_seeds"), None);
        assert_eq!(growth_ticks("minecraft:soul_sand", "minecraft:nether_wart"), Some(3000));
        assert_eq!(growth_ticks("minecraft:dirt", "minecraft:stick"), None);
    }

    #[test]
    fn a_crop_is_ripe_after_its_ticks_and_progress_is_lost_if_the_soil_is_taken() {
        let mut p = 0;
        let mut ticks = 0;
        loop {
            ticks += 1;
            let (np, ripe) = tick(p, Some("minecraft:dirt"), Some("minecraft:carrot"));
            p = np;
            if ripe {
                break;
            }
        }
        assert_eq!(ticks, 2400);
        assert_eq!(tick(1500, None, Some("minecraft:carrot")), (0, false));
    }

    #[test]
    fn a_harvest_gives_counts_within_range_and_a_100_percent_drop_always() {
        let crop = crop_of("minecraft:melon_seeds").unwrap();
        for t in 0..300 {
            let got = harvest(crop, roll_stream(t, [1, 2, 3]));
            assert_eq!(got.len(), 1);
            assert!((3..=7).contains(&got[0].1), "{got:?}");
        }
    }

    #[test]
    fn a_chance_drop_comes_about_as_often_as_its_percentage() {
        let crop = crop_of("minecraft:oak_sapling").unwrap();
        let with_apple = (0..4000)
            .filter(|t| harvest(crop, roll_stream(*t, [0, 0, 0])).iter().any(|(i, _)| *i == "minecraft:apple"))
            .count();
        assert!((100..=320).contains(&with_apple), "{with_apple}");
    }
}
