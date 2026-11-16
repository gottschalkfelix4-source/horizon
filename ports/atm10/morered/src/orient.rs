//! Orientation of a block that sits on one face of its neighbour: which face it is attached to,
//! which way its output points, and where its left, right and back are. Pure maths, no host.
use mod_api::Direction;

/// The six directions in `Direction::ALL` order (down, up, north, south, west, east), also the
/// bit order of a wire's face mask.
pub const ALL: [Direction; 6] = Direction::ALL;

/// The state-property name of a direction.
pub fn name(d: Direction) -> &'static str {
    match d {
        Direction::Down => "down",
        Direction::Up => "up",
        Direction::North => "north",
        Direction::South => "south",
        Direction::West => "west",
        Direction::East => "east",
    }
}

/// The direction a state-property value names.
pub fn parse(s: &str) -> Option<Direction> {
    ALL.into_iter().find(|d| name(*d) == s)
}

/// The bit of `d` in a face mask.
pub fn bit(d: Direction) -> u8 {
    1 << ALL.iter().position(|x| *x == d).unwrap_or(0)
}

/// `pos` moved one block towards `d`.
pub fn step(pos: [i32; 3], d: Direction) -> [i32; 3] {
    let n = d.normal();
    [pos[0] + n[0], pos[1] + n[1], pos[2] + n[2]]
}

fn from_normal(n: [i32; 3]) -> Option<Direction> {
    ALL.into_iter().find(|d| d.normal() == n)
}

/// The four directions an output can point for a block attached on `attach`, in rotation order.
/// Rotation 0 is north on a floor or ceiling and up on a wall; each next one turns a quarter.
pub fn perpendiculars(attach: Direction) -> [Direction; 4] {
    use Direction::*;
    match attach {
        Down | Up => [North, East, South, West],
        North | South => [Up, East, Down, West],
        West | East => [Up, North, Down, South],
    }
}

/// Where a gate's sides are, seen from above the face it is attached to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sides {
    pub output: Direction,
    pub back: Direction,
    pub left: Direction,
    pub right: Direction,
}

/// The sides of a block attached on `attach` with output rotation `rotation` (0 to 3).
pub fn sides(attach: Direction, rotation: u8) -> Sides {
    let output = perpendiculars(attach)[usize::from(rotation & 3)];
    // Left is the output turned a quarter anticlockwise about the plate's upward normal.
    let up = attach.opposite().normal();
    let o = output.normal();
    let left = [
        up[1] * o[2] - up[2] * o[1],
        up[2] * o[0] - up[0] * o[2],
        up[0] * o[1] - up[1] * o[0],
    ];
    let left = from_normal(left).unwrap_or(output);
    Sides {
        output,
        back: output.opposite(),
        left,
        right: left.opposite(),
    }
}

/// The unit vector a player looks along, from yaw and pitch in degrees (`Entity.getViewVector`).
pub fn look(yaw: f32, pitch: f32) -> [f64; 3] {
    let (yaw, pitch) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    [
        -yaw.sin() * pitch.cos(),
        -pitch.sin(),
        yaw.cos() * pitch.cos(),
    ]
}

/// The rotation whose output points closest to where the player looks: a gate placed on a floor
/// faces away from the player, as a repeater does.
pub fn rotation_towards(attach: Direction, look: [f64; 3]) -> u8 {
    let mut best = (0u8, f64::MIN);
    for (i, d) in perpendiculars(attach).into_iter().enumerate() {
        let n = d.normal();
        let dot = f64::from(n[0]) * look[0] + f64::from(n[1]) * look[1] + f64::from(n[2]) * look[2];
        if dot > best.1 {
            best = (i as u8, dot);
        }
    }
    best.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use Direction::*;

    #[test]
    fn a_floor_gate_facing_north_has_west_on_its_left() {
        let s = sides(Down, 0);
        assert_eq!(
            s,
            Sides {
                output: North,
                back: South,
                left: West,
                right: East
            }
        );
    }

    #[test]
    fn a_ceiling_gate_facing_north_has_its_left_and_right_swapped() {
        // Seen from below, west is on the right of someone looking north.
        let s = sides(Up, 0);
        assert_eq!((s.left, s.right), (East, West));
    }

    #[test]
    fn a_wall_gate_pointing_up_has_its_sides_horizontal() {
        // Attached to the block to its north, seen from the south, output up.
        let s = sides(North, 0);
        assert_eq!(s.output, Up);
        assert_eq!(s.back, Down);
        assert_eq!((s.left, s.right), (West, East));
    }

    #[test]
    fn the_placer_looking_east_at_a_floor_turns_the_output_east() {
        // Yaw -90 looks towards +x (east) in Minecraft's convention.
        assert_eq!(
            perpendiculars(Down)[rotation_towards(Down, look(-90.0, 30.0)) as usize],
            East
        );
        assert_eq!(
            perpendiculars(Down)[rotation_towards(Down, look(0.0, 10.0)) as usize],
            South
        );
    }

    #[test]
    fn on_a_wall_looking_down_turns_the_output_down() {
        assert_eq!(
            perpendiculars(East)[rotation_towards(East, look(90.0, 80.0)) as usize],
            Down
        );
    }
}
