//! Bridging Mod for Horizon: reach-around placement for bridging. Standing on the edge of a
//! block and looking down past it, the crosshair hits nothing; Bridging Mod lets the player place
//! against the side of the block under their feet anyway, and outlines where the block goes.
//!
//! Written from Bridging Mod's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, what it can't yet, and what a check
//! against the NeoForge original still has to confirm, is in README.md.
//!
//! This port finds the target (`target`) and outlines it each frame. Placing the block there
//! needs a client call the API doesn't have yet (README, "Needs from Horizon").
use mod_api::{
    ClientHost, Direction, Hooks, RenderLevelStage, RenderLevelStageEvent, WorldDepth, WorldLine,
};

/// The lowest pitch (degrees, down positive) at which reach-around applies: the player has to
/// look down past the edge.
pub const MIN_PITCH: f32 = 25.0;

/// The outline's colour (0xAARRGGBB): vanilla's block outline, 40% black.
pub const OUTLINE: u32 = 0x6600_0000;

/// Where a reach-around click places.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    /// The block the player stands on, clicked on its side.
    pub support: [i32; 3],
    /// The side of `support` the click lands on.
    pub face: Direction,
    /// The empty cell the block goes in.
    pub place: [i32; 3],
}

/// The horizontal direction a yaw (degrees) faces, as `Direction.fromYRot`.
pub fn facing(yaw: f32) -> Direction {
    let quarter = ((f64::from(yaw) / 90.0) + 0.5).floor() as i64;
    match quarter.rem_euclid(4) {
        0 => Direction::South,
        1 => Direction::West,
        2 => Direction::North,
        _ => Direction::East,
    }
}

/// The reach-around target for a player whose feet are at `feet`, looking `yaw`/`pitch`, when the
/// crosshair hits nothing. `solid(pos)` says whether a cell has a collision shape. None when the
/// player isn't looking down far enough, stands on nothing, or the cell in front is taken.
pub fn target(
    feet: [f64; 3],
    yaw: f32,
    pitch: f32,
    crosshair_miss: bool,
    solid: impl Fn([i32; 3]) -> bool,
) -> Option<Target> {
    if !crosshair_miss || pitch < MIN_PITCH {
        return None;
    }
    // The block under the feet: a hair below them, so standing exactly on a block's top counts.
    let support = [
        feet[0].floor() as i32,
        (feet[1] - 1.0e-3).floor() as i32,
        feet[2].floor() as i32,
    ];
    if !solid(support) {
        return None;
    }
    let face = facing(yaw);
    let n = face.normal();
    let place = [support[0] + n[0], support[1] + n[1], support[2] + n[2]];
    if solid(place) {
        return None;
    }
    Some(Target {
        support,
        face,
        place,
    })
}

/// The twelve edges of the unit cell at `pos`, grown a little so they sit just outside it.
pub fn cell_edges(pos: [i32; 3]) -> [([f64; 3], [f64; 3]); 12] {
    const G: f64 = 0.002;
    let lo = [
        f64::from(pos[0]) - G,
        f64::from(pos[1]) - G,
        f64::from(pos[2]) - G,
    ];
    let hi = [
        lo[0] + 1.0 + 2.0 * G,
        lo[1] + 1.0 + 2.0 * G,
        lo[2] + 1.0 + 2.0 * G,
    ];
    let c = |x: bool, y: bool, z: bool| {
        [
            if x { hi[0] } else { lo[0] },
            if y { hi[1] } else { lo[1] },
            if z { hi[2] } else { lo[2] },
        ]
    };
    let mut out = [([0.0; 3], [0.0; 3]); 12];
    let mut i = 0;
    for a in [false, true] {
        for b in [false, true] {
            out[i] = (c(false, a, b), c(true, a, b));
            out[i + 1] = (c(a, false, b), c(a, true, b));
            out[i + 2] = (c(a, b, false), c(a, b, true));
            i += 3;
        }
    }
    out
}

fn current_target(host: &dyn ClientHost) -> Option<Target> {
    let camera = host.camera()?;
    let player = host.camera_entity()?;
    let feet = player.position(host)?;
    let miss = host.crosshair_target().is_none();
    target(feet, camera.yaw, camera.pitch, miss, |p| {
        !host.block_collision(p).is_empty()
    })
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Outline the cell a reach-around click would fill.
    fn render_level_stage(&self, host: &dyn ClientHost, event: &mut RenderLevelStageEvent<'_>) {
        if event.stage != RenderLevelStage::AfterLevel {
            return;
        }
        let Some(t) = current_target(host) else {
            return;
        };
        let lines = cell_edges(t.place)
            .map(|(from, to)| WorldLine::new(from, to, OUTLINE, 2.0, WorldDepth::Tested));
        let _ = event.draw.lines(&lines);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaw_maps_to_minecrafts_horizontal_directions() {
        assert_eq!(facing(0.0), Direction::South);
        assert_eq!(facing(90.0), Direction::West);
        assert_eq!(facing(-180.0), Direction::North);
        assert_eq!(facing(-90.0), Direction::East);
        assert_eq!(facing(44.0), Direction::South);
        assert_eq!(facing(46.0), Direction::West);
    }

    #[test]
    fn looking_down_past_the_edge_targets_the_cell_beside_the_block_underfoot() {
        // Standing on the block at (0,63,0), facing north (yaw 180), looking down.
        let floor = |p: [i32; 3]| p == [0, 63, 0];
        let t = target([0.5, 64.0, 0.1], 180.0, 70.0, true, floor).unwrap();
        assert_eq!(t.support, [0, 63, 0]);
        assert_eq!(t.face, Direction::North);
        assert_eq!(t.place, [0, 63, -1]);
    }

    #[test]
    fn no_target_when_looking_level_aiming_at_a_block_or_the_cell_is_taken() {
        let floor = |p: [i32; 3]| p == [0, 63, 0];
        assert!(target([0.5, 64.0, 0.5], 180.0, 10.0, true, floor).is_none());
        assert!(target([0.5, 64.0, 0.5], 180.0, 70.0, false, floor).is_none());
        let both = |p: [i32; 3]| p == [0, 63, 0] || p == [0, 63, -1];
        assert!(target([0.5, 64.0, 0.5], 180.0, 70.0, true, both).is_none());
        assert!(target([0.5, 64.0, 0.5], 180.0, 70.0, true, |_| false).is_none());
    }

    #[test]
    fn the_outline_has_twelve_unit_edges_around_the_cell() {
        let edges = cell_edges([2, 5, -3]);
        for (a, b) in edges {
            let len: f64 = (0..3).map(|i| (b[i] - a[i]).abs()).sum();
            assert!((len - 1.004).abs() < 1e-9);
            assert!(a[0] >= 1.99 && b[0] <= 3.01);
        }
    }
}
