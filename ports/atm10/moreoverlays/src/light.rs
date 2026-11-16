//! The light level overlay: which blocks a monster could spawn on, and when.
//!
//! A cross is drawn on a block's top when a monster could stand in the space above it: red when
//! it is dark enough there at any time of day, yellow when only the night sky makes it dark
//! enough. Written from the mod's visible behaviour and from vanilla's spawn rules (which Horizon
//! already ports), never from More Overlays' code.

/// What the overlay marks on a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// Monsters can spawn here day and night.
    Red,
    /// Monsters can spawn here only at night (or in a thunderstorm).
    Yellow,
}

impl Mark {
    /// The cross's colour, 0xAARRGGBB.
    pub fn color(self) -> u32 {
        match self {
            Mark::Red => 0xFFFF_0000,
            Mark::Yellow => 0xFFFF_FF00,
        }
    }
}

/// The light rules of the dimension the player is in (its dimension type).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// `monster_spawn_block_light_limit`: block light above it never spawns a monster
    /// (0 in the Overworld, 15 in the Nether).
    pub block_limit: i32,
    /// The upper end of `monster_spawn_light_level` (0..7 in the Overworld): a brightness above
    /// it never spawns a monster.
    pub max_level: i32,
    /// The dimension has a sky that darkens at night.
    pub has_sky: bool,
}

impl Limits {
    /// The Overworld's rules, used until the dimension type is known.
    pub const OVERWORLD: Limits = Limits {
        block_limit: 0,
        max_level: 7,
        has_sky: true,
    };
}

/// How much the sky light drops at midnight (`Level.getSkyDarken` reaches 11).
const NIGHT_DARKEN: i32 = 11;

/// The mark for a space with this block and sky light, or `None` if no monster spawns there.
pub fn classify(block: i32, sky: i32, limits: Limits) -> Option<Mark> {
    if block > limits.block_limit {
        return None;
    }
    let day = block.max(sky);
    if day <= limits.max_level {
        return Some(Mark::Red);
    }
    let night = if limits.has_sky {
        block.max(sky - NIGHT_DARKEN)
    } else {
        day
    };
    (night <= limits.max_level).then_some(Mark::Yellow)
}

/// Blocks no monster spawns on although their top is full (`isValidSpawn` answers no, or only
/// for a few animals): glass, leaves, ice, bedrock, barriers, and blocks that give light 14 or 15.
fn never_spawn_on(name: &str) -> bool {
    let id = name.split('[').next().unwrap_or(name);
    let id = id.strip_prefix("minecraft:").unwrap_or(id);
    let lit_lamp = id == "redstone_lamp" && name.contains("lit=true");
    let full_anchor = id == "respawn_anchor" && name.contains("charges=4");
    lit_lamp
        || full_anchor
        || id == "glass"
        || id == "tinted_glass"
        || id.ends_with("_stained_glass")
        || id.ends_with("_leaves")
        || matches!(
            id,
            "ice"
                | "frosted_ice"
                | "bedrock"
                | "barrier"
                | "magma_block"
                | "glowstone"
                | "sea_lantern"
                | "jack_o_lantern"
                | "shroomlight"
                | "beacon"
                | "ochre_froglight"
                | "verdant_froglight"
                | "pearlescent_froglight"
        )
}

/// Whether boxes (block-local, `[min x, y, z, max x, y, z]`) cover the whole top face of the
/// block. Each box is tested on its own, which is right for every vanilla full-topped shape
/// (slabs, upside-down stairs, soul sand's outline).
pub fn full_top(boxes: &[[f64; 6]]) -> bool {
    const E: f64 = 1e-7;
    boxes
        .iter()
        .any(|b| b[0] <= E && b[2] <= E && b[3] >= 1.0 - E && b[4] >= 1.0 - E && b[5] >= 1.0 - E)
}

/// Whether a box list is a whole block.
pub fn full_block(boxes: &[[f64; 6]]) -> bool {
    const E: f64 = 1e-7;
    boxes.iter().any(|b| {
        b[0] <= E && b[1] <= E && b[2] <= E && b[3] >= 1.0 - E && b[4] >= 1.0 - E && b[5] >= 1.0 - E
    })
}

/// What the overlay knows about one column: the block below the space and the space itself.
#[derive(Clone, Debug, PartialEq)]
pub struct Spot<'a> {
    /// The block below, as its state string (`minecraft:stone`, `minecraft:oak_slab[type=top]`).
    pub below: &'a str,
    /// The block below's outline boxes.
    pub below_shape: &'a [[f64; 6]],
    /// The space's collision boxes.
    pub space_collision: &'a [[f64; 6]],
    /// The space holds a fluid.
    pub space_fluid: bool,
    /// The space's block is in `minecraft:prevent_mob_spawning_inside` (rails).
    pub space_prevents: bool,
}

/// Whether a monster could stand in the space (`isValidSpawn` below, `isValidEmptySpawnBlock`
/// in the space, as far as a client can tell).
pub fn spawnable(spot: &Spot<'_>) -> bool {
    !never_spawn_on(spot.below)
        && full_top(spot.below_shape)
        && !full_block(spot.space_collision)
        && !spot.space_fluid
        && !spot.space_prevents
}

/// The two diagonals of a cross on the top of the block below `space`, slightly above it so
/// the cross is not hidden in the surface.
pub fn cross(space: [i32; 3]) -> [([f64; 3], [f64; 3]); 2] {
    let [x, y, z] = space.map(f64::from);
    let y = y + 0.02;
    [
        ([x, y, z], [x + 1.0, y, z + 1.0]),
        ([x + 1.0, y, z], [x, y, z + 1.0]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUBE: [[f64; 6]; 1] = [[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]];
    const BOTTOM_SLAB: [[f64; 6]; 1] = [[0.0, 0.0, 0.0, 1.0, 0.5, 1.0]];
    const TOP_SLAB: [[f64; 6]; 1] = [[0.0, 0.5, 0.0, 1.0, 1.0, 1.0]];
    const CARPET: [[f64; 6]; 1] = [[0.0, 0.0, 0.0, 1.0, 0.0625, 1.0]];

    fn spot<'a>(below: &'a str, below_shape: &'a [[f64; 6]]) -> Spot<'a> {
        Spot {
            below,
            below_shape,
            space_collision: &[],
            space_fluid: false,
            space_prevents: false,
        }
    }

    #[test]
    fn overworld_marks_follow_block_and_sky_light() {
        let ow = Limits::OVERWORLD;
        // A cave: no light at all.
        assert_eq!(classify(0, 0, ow), Some(Mark::Red));
        // Open grass at noon: full sky light, dark only at night.
        assert_eq!(classify(0, 15, ow), Some(Mark::Yellow));
        // Under an overhang with sky light 7: dark enough even by day.
        assert_eq!(classify(0, 7, ow), Some(Mark::Red));
        // A torch's glow (any block light) stops Overworld spawns since 1.18.
        assert_eq!(classify(1, 0, ow), None);
    }

    #[test]
    fn nether_marks_only_dark_spaces() {
        // The Nether: block light up to 15 allowed, brightness must be 7 or less.
        let nether = Limits {
            block_limit: 15,
            max_level: 7,
            has_sky: false,
        };
        assert_eq!(classify(7, 0, nether), Some(Mark::Red));
        assert_eq!(classify(8, 0, nether), None);
    }

    #[test]
    fn surfaces_need_a_full_top_and_a_valid_block() {
        assert!(spawnable(&spot("minecraft:stone", &CUBE)));
        assert!(spawnable(&spot(
            "minecraft:oak_slab[type=top,waterlogged=false]",
            &TOP_SLAB
        )));
        assert!(!spawnable(&spot(
            "minecraft:oak_slab[type=bottom,waterlogged=false]",
            &BOTTOM_SLAB
        )));
        assert!(!spawnable(&spot("minecraft:glass", &CUBE)));
        assert!(!spawnable(&spot(
            "minecraft:oak_leaves[distance=7,persistent=true]",
            &CUBE
        )));
        assert!(!spawnable(&spot("minecraft:glowstone", &CUBE)));
        assert!(!spawnable(&spot(
            "minecraft:redstone_lamp[lit=true]",
            &CUBE
        )));
        assert!(spawnable(&spot(
            "minecraft:redstone_lamp[lit=false]",
            &CUBE
        )));
    }

    #[test]
    fn the_space_must_be_open_dry_and_not_a_rail() {
        let carpet = Spot {
            space_collision: &CARPET,
            ..spot("minecraft:stone", &CUBE)
        };
        assert!(spawnable(&carpet));
        let filled = Spot {
            space_collision: &CUBE,
            ..spot("minecraft:stone", &CUBE)
        };
        assert!(!spawnable(&filled));
        let water = Spot {
            space_fluid: true,
            ..spot("minecraft:stone", &CUBE)
        };
        assert!(!spawnable(&water));
        let rail = Spot {
            space_prevents: true,
            ..spot("minecraft:stone", &CUBE)
        };
        assert!(!spawnable(&rail));
    }
}
