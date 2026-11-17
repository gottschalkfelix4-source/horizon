//! The millstone's and crushing wheel's recipes, from the original's recipe data (Create 6.0.10,
//! the core entries without other mods' items): inputs, outputs with their chances, and the
//! time in ticks at 16 RPM. Free of the game so it can be tested.

/// One output: item id, count, chance (1.0 always).
pub type Out = (&'static str, u32, f32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recipe {
    pub outputs: &'static [Out],
    pub time: u32,
}

const XP: &str = "create:experience_nugget";

/// Milling (the millstone).
pub fn milling(id: &str) -> Option<Recipe> {
    macro_rules! r {
        ($t:expr, $($o:expr),+) => { Recipe { outputs: &[$($o),+], time: $t } };
    }
    Some(match id {
        "minecraft:andesite" => r!(200, ("minecraft:cobblestone", 1, 1.0)),
        "minecraft:cobblestone" => r!(250, ("minecraft:gravel", 1, 1.0)),
        "minecraft:gravel" => r!(250, ("minecraft:flint", 1, 1.0)),
        "minecraft:granite" | "minecraft:terracotta" => r!(200, ("minecraft:red_sand", 1, 1.0)),
        "minecraft:sandstone" => r!(150, ("minecraft:sand", 1, 1.0)),
        "minecraft:clay" => r!(50, ("minecraft:clay_ball", 4, 1.0)),
        "minecraft:dripstone_block" => r!(250, ("minecraft:clay_ball", 1, 1.0)),
        "minecraft:calcite" => r!(250, ("minecraft:bone_meal", 1, 0.75)),
        "minecraft:wheat" => r!(150, ("create:wheat_flour", 1, 1.0), ("create:wheat_flour", 2, 0.25), ("minecraft:wheat_seeds", 1, 0.25)),
        "minecraft:bone" => r!(100, ("minecraft:bone_meal", 3, 1.0), ("minecraft:white_dye", 1, 0.25), ("minecraft:bone_meal", 3, 0.25)),
        "minecraft:bone_meal" => r!(70, ("minecraft:white_dye", 2, 1.0), ("minecraft:light_gray_dye", 1, 0.1)),
        "minecraft:coal" | "minecraft:ink_sac" => r!(100, ("minecraft:black_dye", 2, 1.0), ("minecraft:gray_dye", 1, 0.1)),
        "minecraft:charcoal" => r!(100, ("minecraft:black_dye", 1, 1.0), ("minecraft:gray_dye", 2, 0.1)),
        "minecraft:lapis_lazuli" => r!(100, ("minecraft:blue_dye", 2, 1.0), ("minecraft:blue_dye", 1, 0.1)),
        "minecraft:saddle" => r!(200, ("minecraft:leather", 2, 1.0), ("minecraft:leather", 2, 0.5)),
        "minecraft:sugar_cane" => r!(50, ("minecraft:sugar", 2, 1.0), ("minecraft:sugar", 1, 0.1)),
        "minecraft:short_grass" => r!(50, ("minecraft:wheat_seeds", 1, 0.25)),
        "minecraft:tall_grass" => r!(100, ("minecraft:wheat_seeds", 1, 0.5)),
        "minecraft:fern" => r!(50, ("minecraft:green_dye", 1, 1.0), ("minecraft:wheat_seeds", 1, 0.1)),
        "minecraft:large_fern" => r!(50, ("minecraft:green_dye", 2, 1.0), ("minecraft:green_dye", 1, 0.5), ("minecraft:wheat_seeds", 1, 0.1)),
        "minecraft:beetroot" => r!(70, ("minecraft:red_dye", 2, 1.0), ("minecraft:beetroot_seeds", 1, 0.1)),
        "minecraft:cocoa_beans" => r!(70, ("minecraft:brown_dye", 2, 1.0), ("minecraft:brown_dye", 1, 0.1)),
        "minecraft:cactus" => r!(50, ("minecraft:green_dye", 2, 1.0), ("minecraft:green_dye", 1, 0.1)),
        "minecraft:sea_pickle" => r!(50, ("minecraft:lime_dye", 2, 1.0), ("minecraft:green_dye", 1, 0.1)),
        "minecraft:poppy" => r!(50, ("minecraft:red_dye", 2, 1.0), ("minecraft:green_dye", 1, 0.05)),
        "minecraft:dandelion" => r!(50, ("minecraft:yellow_dye", 2, 1.0), ("minecraft:yellow_dye", 1, 0.05)),
        "minecraft:cornflower" => r!(50, ("minecraft:blue_dye", 2, 1.0)),
        "minecraft:wither_rose" => r!(50, ("minecraft:black_dye", 2, 1.0), ("minecraft:black_dye", 1, 0.1)),
        id if id.starts_with("minecraft:") && id.ends_with("_wool") => r!(100, ("minecraft:string", 1, 1.0)),
        _ => return None,
    })
}

/// Crushing (the crushing wheel pair).
pub fn crushing(id: &str) -> Option<Recipe> {
    macro_rules! r {
        ($t:expr, $($o:expr),+) => { Recipe { outputs: &[$($o),+], time: $t } };
    }
    Some(match id {
        "minecraft:iron_ore" => r!(250, ("create:crushed_raw_iron", 1, 1.0), ("create:crushed_raw_iron", 1, 0.75), (XP, 1, 0.75), ("minecraft:cobblestone", 1, 0.125)),
        "minecraft:copper_ore" => r!(250, ("create:crushed_raw_copper", 5, 1.0), ("create:crushed_raw_copper", 1, 0.25), (XP, 1, 0.75), ("minecraft:cobblestone", 1, 0.125)),
        "minecraft:gold_ore" => r!(250, ("create:crushed_raw_gold", 1, 1.0), ("create:crushed_raw_gold", 1, 0.75), (XP, 2, 0.75), ("minecraft:cobblestone", 1, 0.125)),
        "create:zinc_ore" => r!(250, ("create:crushed_raw_zinc", 1, 1.0), ("create:crushed_raw_zinc", 1, 0.75), (XP, 1, 0.75), ("minecraft:cobblestone", 1, 0.125)),
        "minecraft:coal_ore" => r!(150, ("minecraft:coal", 1, 1.0), ("minecraft:coal", 1, 0.75), (XP, 1, 0.75), ("minecraft:cobblestone", 1, 0.125)),
        "minecraft:lapis_ore" => r!(250, ("minecraft:lapis_lazuli", 10, 1.0), ("minecraft:lapis_lazuli", 1, 0.5), (XP, 1, 0.75), ("minecraft:cobblestone", 1, 0.125)),
        "minecraft:redstone_ore" => r!(250, ("minecraft:redstone", 6, 1.0), ("minecraft:redstone", 1, 0.5), (XP, 1, 0.75), ("minecraft:cobblestone", 1, 0.125)),
        "minecraft:diamond_ore" => r!(350, ("minecraft:diamond", 1, 1.0), ("minecraft:diamond", 1, 0.75), (XP, 1, 0.75), ("minecraft:cobblestone", 1, 0.125)),
        "minecraft:emerald_ore" => r!(350, ("minecraft:emerald", 1, 1.0), ("minecraft:emerald", 1, 0.75), (XP, 1, 0.75), ("minecraft:cobblestone", 1, 0.125)),
        "minecraft:nether_gold_ore" => r!(350, ("minecraft:gold_nugget", 18, 1.0), (XP, 1, 0.75), ("minecraft:netherrack", 1, 0.125)),
        "minecraft:nether_quartz_ore" => r!(350, ("minecraft:quartz", 2, 1.0), ("minecraft:quartz", 1, 0.25), (XP, 1, 0.75), ("minecraft:netherrack", 1, 0.125)),
        "minecraft:gilded_blackstone" => r!(400, ("minecraft:gold_nugget", 18, 1.0), (XP, 1, 0.75), ("minecraft:blackstone", 1, 0.125)),
        "minecraft:deepslate_coal_ore" => r!(300, ("minecraft:coal", 2, 1.0), ("minecraft:coal", 1, 0.25), (XP, 1, 0.75), ("minecraft:cobbled_deepslate", 1, 0.125)),
        "minecraft:deepslate_copper_ore" => r!(350, ("create:crushed_raw_copper", 7, 1.0), ("create:crushed_raw_copper", 1, 0.25), (XP, 1, 0.75), ("minecraft:cobbled_deepslate", 1, 0.125)),
        "minecraft:deepslate_iron_ore" => r!(350, ("create:crushed_raw_iron", 2, 1.0), ("create:crushed_raw_iron", 1, 0.25), (XP, 1, 0.75), ("minecraft:cobbled_deepslate", 1, 0.125)),
        "minecraft:deepslate_gold_ore" => r!(350, ("create:crushed_raw_gold", 2, 1.0), ("create:crushed_raw_gold", 1, 0.25), (XP, 2, 0.75), ("minecraft:cobbled_deepslate", 1, 0.125)),
        "create:deepslate_zinc_ore" => r!(350, ("create:crushed_raw_zinc", 2, 1.0), ("create:crushed_raw_zinc", 1, 0.25), (XP, 1, 0.75), ("minecraft:cobbled_deepslate", 1, 0.125)),
        "minecraft:deepslate_lapis_ore" => r!(350, ("minecraft:lapis_lazuli", 12, 1.0), ("minecraft:lapis_lazuli", 1, 0.5), (XP, 1, 0.75), ("minecraft:cobbled_deepslate", 1, 0.125)),
        "minecraft:deepslate_redstone_ore" => r!(350, ("minecraft:redstone", 7, 1.0), ("minecraft:redstone", 1, 0.5), (XP, 1, 0.75), ("minecraft:cobbled_deepslate", 1, 0.125)),
        "minecraft:deepslate_diamond_ore" => r!(450, ("minecraft:diamond", 2, 1.0), ("minecraft:diamond", 1, 0.25), (XP, 1, 0.75), ("minecraft:cobbled_deepslate", 1, 0.125)),
        "minecraft:deepslate_emerald_ore" => r!(450, ("minecraft:emerald", 2, 1.0), ("minecraft:emerald", 1, 0.25), (XP, 1, 0.75), ("minecraft:cobbled_deepslate", 1, 0.125)),
        "minecraft:raw_iron" => r!(400, ("create:crushed_raw_iron", 1, 1.0), (XP, 1, 0.75)),
        "minecraft:raw_copper" => r!(400, ("create:crushed_raw_copper", 1, 1.0), (XP, 1, 0.75)),
        "minecraft:raw_gold" => r!(400, ("create:crushed_raw_gold", 1, 1.0), (XP, 2, 0.75)),
        "create:raw_zinc" => r!(400, ("create:crushed_raw_zinc", 1, 1.0), (XP, 1, 0.75)),
        "minecraft:raw_iron_block" => r!(400, ("create:crushed_raw_iron", 9, 1.0), (XP, 9, 0.75)),
        "minecraft:raw_copper_block" => r!(400, ("create:crushed_raw_copper", 9, 1.0), (XP, 9, 0.75)),
        "minecraft:raw_gold_block" => r!(400, ("create:crushed_raw_gold", 9, 1.0), (XP, 18, 0.75)),
        "minecraft:tuff" => r!(350, ("minecraft:flint", 1, 0.25), ("minecraft:gold_nugget", 1, 0.1), ("minecraft:copper_nugget", 1, 0.1), ("create:zinc_nugget", 1, 0.1), ("minecraft:iron_nugget", 1, 0.1)),
        "minecraft:diorite" => r!(350, ("minecraft:quartz", 1, 0.25)),
        "minecraft:gravel" => r!(250, ("minecraft:sand", 1, 1.0), ("minecraft:flint", 1, 0.1), ("minecraft:clay_ball", 1, 0.05)),
        "minecraft:obsidian" => r!(500, ("create:powdered_obsidian", 1, 1.0), ("minecraft:obsidian", 1, 0.75)),
        "minecraft:netherrack" => r!(250, ("create:cinder_flour", 1, 1.0), ("create:cinder_flour", 1, 0.5)),
        "minecraft:amethyst_block" => r!(150, ("minecraft:amethyst_shard", 3, 1.0), ("minecraft:amethyst_shard", 1, 0.5)),
        "minecraft:amethyst_cluster" => r!(150, ("minecraft:amethyst_shard", 7, 1.0), ("minecraft:amethyst_shard", 1, 0.5)),
        "minecraft:glowstone" => r!(150, ("minecraft:glowstone_dust", 3, 1.0), ("minecraft:glowstone_dust", 1, 0.5)),
        "minecraft:blaze_rod" => r!(100, ("minecraft:blaze_powder", 3, 1.0), ("minecraft:blaze_powder", 3, 0.25)),
        "minecraft:prismarine_crystals" => r!(150, ("minecraft:quartz", 1, 1.0), ("minecraft:quartz", 2, 0.5), ("minecraft:glowstone_dust", 2, 0.1)),
        "minecraft:nether_wart_block" => r!(150, ("minecraft:nether_wart", 1, 0.25)),
        "minecraft:leather_horse_armor" => r!(200, ("minecraft:leather", 2, 1.0), ("minecraft:leather", 2, 0.5)),
        id if id.starts_with("minecraft:") && id.ends_with("_wool") => r!(100, ("minecraft:string", 2, 1.0), ("minecraft:string", 1, 0.5)),
        _ => return None,
    })
}

/// A number in 0..1 from a position and the time: the same inputs give the same roll, different
/// ones spread well enough for drop chances.
pub fn roll(pos: [i32; 3], time: i64, salt: u32) -> f32 {
    let mut h = (pos[0] as i64).wrapping_mul(73_856_093)
        ^ (pos[1] as i64).wrapping_mul(19_349_663)
        ^ (pos[2] as i64).wrapping_mul(83_492_791)
        ^ time.wrapping_mul(2_654_435_761)
        ^ i64::from(salt).wrapping_mul(40_503);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    ((h as u64 % 1_000_000) as f32) / 1_000_000.0
}

/// The outputs that drop for this roll: every sure one, and a chance one when its roll is under it.
pub fn drops(r: &Recipe, pos: [i32; 3], time: i64) -> Vec<(&'static str, u32)> {
    r.outputs
        .iter()
        .enumerate()
        .filter(|(i, o)| o.2 >= 1.0 || roll(pos, time, *i as u32) < o.2)
        .map(|(_, o)| (o.0, o.1))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iron_ore_takes_250_ticks_and_always_gives_one_crushed_ore() {
        let r = crushing("minecraft:iron_ore").unwrap();
        assert_eq!(r.time, 250);
        assert_eq!(r.outputs[0], ("create:crushed_raw_iron", 1, 1.0));
        assert!(drops(&r, [1, 2, 3], 7).iter().any(|d| d.0 == "create:crushed_raw_iron"));
    }

    #[test]
    fn chances_come_out_about_right() {
        let r = crushing("minecraft:iron_ore").unwrap();
        let n = 4000;
        let xp = (0..n).filter(|t| drops(&r, [5, 64, -9], *t).iter().any(|d| d.0 == XP)).count();
        let share = xp as f32 / n as f32;
        assert!((share - 0.75).abs() < 0.05, "{share}");
    }

    #[test]
    fn a_wool_block_mills_to_one_string() {
        assert_eq!(milling("minecraft:red_wool").unwrap().outputs, &[("minecraft:string", 1, 1.0)]);
        assert!(milling("minecraft:dirt").is_none());
    }
}
