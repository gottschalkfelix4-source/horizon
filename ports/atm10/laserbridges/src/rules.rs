//! Where a bridge goes, apart from the game.

/// Longest bridge, in blocks.
pub const RANGE: usize = 32;

/// What a cell on the beam's line holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Air,
    /// A beam block of this mod.
    Beam,
    Receiver,
    /// Anything else: stops the beam.
    Solid,
}

/// What a line of cells (the first is next to the emitter) makes of a lit emitter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bridge {
    /// How many cells from the emitter are beam.
    pub length: usize,
    /// Whether the beam ends on a receiver.
    pub receiver: bool,
}

/// The beam crosses air and beam blocks (not fluids or plants: those are solid to it) up to
/// [`RANGE`], and stops on the first cell that is neither; a receiver there is lit.
pub fn bridge(line: impl Iterator<Item = Cell>) -> Bridge {
    let mut length = 0;
    for cell in line.take(RANGE + 1) {
        match cell {
            Cell::Air | Cell::Beam if length < RANGE => length += 1,
            Cell::Receiver => return Bridge { length, receiver: true },
            _ => break,
        }
    }
    Bridge { length, receiver: false }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Cell::*;

    #[test]
    fn a_beam_crosses_air_and_old_beam_and_stops_at_a_wall() {
        let b = bridge([Air, Beam, Air, Solid, Air].into_iter());
        assert_eq!(b, Bridge { length: 3, receiver: false });
    }

    #[test]
    fn a_receiver_at_the_end_is_reached_and_one_beyond_a_wall_is_not() {
        assert_eq!(bridge([Air, Air, Receiver].into_iter()), Bridge { length: 2, receiver: true });
        assert!(!bridge([Air, Solid, Receiver].into_iter()).receiver);
    }

    #[test]
    fn the_beam_is_32_long_at_most_and_a_receiver_right_after_is_still_reached() {
        assert_eq!(bridge(std::iter::repeat(Air)).length, RANGE);
        let line = std::iter::repeat(Air).take(RANGE).chain([Receiver]);
        assert_eq!(bridge(line), Bridge { length: RANGE, receiver: true });
    }
}
