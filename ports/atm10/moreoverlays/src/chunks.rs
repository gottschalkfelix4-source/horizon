//! The chunk boundary overlay: lines along chunk (and region) edges around the player.
//!
//! The toggle key cycles Off → Corners → Grid → Regions → Off. What each mode shows is written
//! from the mod's visible behaviour; the exact colours and grid spacing still need checking
//! against the original (README).

/// What the chunk overlay shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    /// Nothing.
    #[default]
    Off,
    /// Vertical lines at the corners of the player's chunk and its eight neighbours.
    Corners,
    /// Corners, plus a grid on the walls of the player's chunk.
    Grid,
    /// Corners, plus the corners of the player's region (32 x 32 chunks).
    Regions,
}

impl Mode {
    /// The mode after this one when the key is pressed.
    pub fn next(self) -> Self {
        match self {
            Mode::Off => Mode::Corners,
            Mode::Corners => Mode::Grid,
            Mode::Grid => Mode::Regions,
            Mode::Regions => Mode::Off,
        }
    }
}

/// A line from, to, and its colour.
pub type Line = ([f64; 3], [f64; 3], u32);

/// The player's chunk's corners.
pub const CHUNK_COLOR: u32 = 0xFFFF_3F3F;
/// The neighbouring chunks' corners.
pub const NEIGHBOR_COLOR: u32 = 0xFF3F_3FFF;
/// The grid on the player's chunk's walls.
pub const GRID_COLOR: u32 = 0xFFFF_FF3F;
/// A region's corners.
pub const REGION_COLOR: u32 = 0xFF3F_FF3F;

/// The lines `mode` draws for a player at block `feet`, in a world from `min_y` to `max_y`
/// (exclusive).
pub fn lines(mode: Mode, feet: [i32; 3], min_y: i32, max_y: i32) -> Vec<Line> {
    if mode == Mode::Off {
        return Vec::new();
    }
    let (bottom, top) = (f64::from(min_y), f64::from(max_y));
    let cx = feet[0].div_euclid(16) * 16;
    let cz = feet[2].div_euclid(16) * 16;
    let vertical = |x: i32, z: i32, color| {
        let (x, z) = (f64::from(x), f64::from(z));
        ([x, bottom, z], [x, top, z], color)
    };
    let mut out = Vec::new();
    for gx in -1..=2 {
        for gz in -1..=2 {
            let own = (0..=1).contains(&gx) && (0..=1).contains(&gz);
            let color = if own { CHUNK_COLOR } else { NEIGHBOR_COLOR };
            out.push(vertical(cx + gx * 16, cz + gz * 16, color));
        }
    }
    match mode {
        Mode::Grid => {
            // Every second block along each wall, and every second block in height within 16
            // blocks of the player's feet, as the vanilla chunk border debug view does.
            for i in (2..16).step_by(2) {
                out.push(vertical(cx + i, cz, GRID_COLOR));
                out.push(vertical(cx + i, cz + 16, GRID_COLOR));
                out.push(vertical(cx, cz + i, GRID_COLOR));
                out.push(vertical(cx + 16, cz + i, GRID_COLOR));
            }
            let from = (feet[1] - 16).max(min_y).div_euclid(2) * 2;
            let to = (feet[1] + 16).min(max_y);
            let (x0, z0) = (f64::from(cx), f64::from(cz));
            let (x1, z1) = (x0 + 16.0, z0 + 16.0);
            for y in (from..=to).step_by(2) {
                let y = f64::from(y);
                out.push(([x0, y, z0], [x1, y, z0], GRID_COLOR));
                out.push(([x1, y, z0], [x1, y, z1], GRID_COLOR));
                out.push(([x1, y, z1], [x0, y, z1], GRID_COLOR));
                out.push(([x0, y, z1], [x0, y, z0], GRID_COLOR));
            }
        }
        Mode::Regions => {
            let rx = feet[0].div_euclid(512) * 512;
            let rz = feet[2].div_euclid(512) * 512;
            for (x, z) in [(0, 0), (512, 0), (0, 512), (512, 512)] {
                out.push(vertical(rx + x, rz + z, REGION_COLOR));
            }
        }
        Mode::Off | Mode::Corners => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_cycles_through_every_mode_and_back_to_off() {
        let mut mode = Mode::Off;
        let mut seen = Vec::new();
        for _ in 0..4 {
            mode = mode.next();
            seen.push(mode);
        }
        assert_eq!(seen, [Mode::Corners, Mode::Grid, Mode::Regions, Mode::Off]);
    }

    #[test]
    fn corners_stand_on_chunk_edges_around_negative_coordinates() {
        // Feet at (-1, 70, 17): chunk x -1 (blocks -16..0), chunk z 1 (blocks 16..32).
        let corners = lines(Mode::Corners, [-1, 70, 17], -64, 320);
        let own: Vec<_> = corners
            .iter()
            .filter(|l| l.2 == CHUNK_COLOR)
            .map(|l| (l.0[0], l.0[2]))
            .collect();
        assert_eq!(
            own,
            [(-16.0, 16.0), (-16.0, 32.0), (0.0, 16.0), (0.0, 32.0)]
        );
        assert_eq!(corners.len(), 16);
        assert!(corners.iter().all(|l| l.0[1] == -64.0 && l.1[1] == 320.0));
    }

    #[test]
    fn regions_mark_the_512_block_region_corners() {
        let regions = lines(Mode::Regions, [600, 64, -5], 0, 256);
        let corners: Vec<_> = regions
            .iter()
            .filter(|l| l.2 == REGION_COLOR)
            .map(|l| (l.0[0], l.0[2]))
            .collect();
        assert_eq!(
            corners,
            [
                (512.0, -512.0),
                (1024.0, -512.0),
                (512.0, 0.0),
                (1024.0, 0.0)
            ]
        );
    }
}
