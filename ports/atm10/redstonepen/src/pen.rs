//! What a click with the pen or quill draws. Pure logic, no host.
use mod_api::Direction;

use crate::orient::{bit, step};

/// What is in the cell a track would go in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    /// Nothing: a new track can go there.
    Air,
    /// A track with these faces (`orient::bit`).
    Track(u8),
    /// Any other block.
    Other,
}

/// What a click does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Draw {
    /// Put a track with `faces` in the cell at `pos`.
    Track { pos: [i32; 3], faces: u8 },
    /// Nothing to draw there.
    Nothing,
}

/// The click on the face `face` of the block at `pos`, with `cell` what is in front of that
/// face: the track lies on the clicked face, in the cell in front of it.
pub fn draw(pos: [i32; 3], face: Direction, cell: Cell) -> Draw {
    let target = step(pos, face);
    let b = bit(face.opposite());
    match cell {
        Cell::Air => Draw::Track {
            pos: target,
            faces: b,
        },
        Cell::Track(faces) if faces & b == 0 => Draw::Track {
            pos: target,
            faces: faces | b,
        },
        _ => Draw::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Direction::*;

    #[test]
    fn clicking_the_top_of_a_block_draws_on_it_in_the_cell_above() {
        assert_eq!(
            draw([3, 64, 3], Up, Cell::Air),
            Draw::Track {
                pos: [3, 65, 3],
                faces: bit(Down)
            }
        );
    }

    #[test]
    fn clicking_a_wall_beside_a_floor_track_adds_the_wall_face_to_that_cell() {
        // A floor track at (4,64,3); its west neighbour's east face is clicked.
        assert_eq!(
            draw([3, 64, 3], East, Cell::Track(bit(Down))),
            Draw::Track {
                pos: [4, 64, 3],
                faces: bit(Down) | bit(West)
            }
        );
    }

    #[test]
    fn a_face_already_drawn_or_a_blocked_cell_draws_nothing() {
        assert_eq!(draw([0, 0, 0], Up, Cell::Track(bit(Down))), Draw::Nothing);
        assert_eq!(draw([0, 0, 0], Up, Cell::Other), Draw::Nothing);
    }
}
