//! XNet's network rules, independent of the game: which cables belong to a controller's
//! network, which connectors are on it, and who gives to whom.
use std::collections::{HashSet, VecDeque};

/// The six directions in vanilla's order: down, up, north, south, west, east.
pub const DIRS: [[i32; 3]; 6] = [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];

pub fn offset(pos: [i32; 3], dir: usize) -> [i32; 3] {
    let d = DIRS[dir];
    [pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]]
}

/// The direction opposite to `dir` (vanilla's order pairs them).
pub fn opposite(dir: usize) -> usize {
    dir ^ 1
}

pub fn dir_of(name: &str) -> Option<usize> {
    ["down", "up", "north", "south", "west", "east"].iter().position(|n| *n == name)
}

/// What a connector moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Items,
    Fluids,
    Energy,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Items, Kind::Fluids, Kind::Energy];
    pub fn index(self) -> i32 {
        Self::ALL.iter().position(|k| *k == self).unwrap_or(0) as i32
    }
    pub fn from_index(i: i32) -> Kind {
        Self::ALL.get(i.max(0) as usize).copied().unwrap_or(Kind::Items)
    }
}

/// What a connector does with the block it faces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Off,
    /// Takes from the block it faces into the network.
    Extract,
    /// Gives what the network carries to the block it faces.
    Insert,
}

impl Mode {
    pub fn index(self) -> i32 {
        match self {
            Mode::Off => 0,
            Mode::Extract => 1,
            Mode::Insert => 2,
        }
    }
    pub fn from_index(i: i32) -> Mode {
        match i {
            1 => Mode::Extract,
            2 => Mode::Insert,
            _ => Mode::Off,
        }
    }
    pub fn next(self) -> Mode {
        Mode::from_index((self.index() + 1) % 3)
    }
}

/// A connector on the network: where it is, the side it faces and what it does there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connector {
    pub pos: [i32; 3],
    /// Direction number of the side it faces (its target is beside it there).
    pub facing: usize,
    pub kind: Kind,
    pub mode: Mode,
    pub channel: u8,
    pub priority: i32,
}

impl Connector {
    pub fn target(&self) -> [i32; 3] {
        offset(self.pos, self.facing)
    }
}

/// The cables connected to `starts` (those that are cables), by flood fill, at most `limit`.
pub fn cables(starts: &[[i32; 3]], mut is_cable: impl FnMut([i32; 3]) -> bool, limit: usize) -> Vec<[i32; 3]> {
    let mut seen: HashSet<[i32; 3]> = HashSet::new();
    let mut queue = VecDeque::new();
    for &s in starts {
        if is_cable(s) && seen.insert(s) {
            queue.push_back(s);
        }
    }
    let mut out = Vec::new();
    while let Some(pos) = queue.pop_front() {
        out.push(pos);
        if out.len() >= limit {
            break;
        }
        for dir in 0..6 {
            let next = offset(pos, dir);
            if !seen.contains(&next) && is_cable(next) {
                seen.insert(next);
                queue.push_back(next);
            }
        }
    }
    out
}

/// Whether a connector at `connector` facing `facing` is attached to the block at `node`
/// (a cable or the controller beside it): it hangs on any side but the one it faces.
pub fn attached(connector: [i32; 3], facing: usize, node: [i32; 3]) -> bool {
    (0..6).any(|dir| offset(connector, dir) == node && dir != facing)
}

/// One delivery plan: the extractor, and the connectors it gives to, best first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    pub from: usize,
    pub to: Vec<usize>,
}

/// Who gives to whom: every extracting connector feeds the inserting connectors of the same
/// channel and kind, highest priority first, then by position, never the block it takes from.
pub fn plan(conns: &[Connector]) -> Vec<Route> {
    let mut routes = Vec::new();
    let mut extractors: Vec<usize> = (0..conns.len()).filter(|&i| conns[i].mode == Mode::Extract).collect();
    extractors.sort_by_key(|&i| conns[i].pos);
    for from in extractors {
        let e = &conns[from];
        let mut to: Vec<usize> = (0..conns.len())
            .filter(|&i| {
                let c = &conns[i];
                c.mode == Mode::Insert && c.channel == e.channel && c.kind == e.kind && c.target() != e.target()
            })
            .collect();
        to.sort_by_key(|&i| (-conns[i].priority, conns[i].pos));
        if !to.is_empty() {
            routes.push(Route { from, to });
        }
    }
    routes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn(pos: [i32; 3], facing: usize, mode: Mode, channel: u8, priority: i32) -> Connector {
        Connector { pos, facing, kind: Kind::Items, mode, channel, priority }
    }

    #[test]
    fn the_flood_fill_follows_touching_cables_only_and_respects_the_limit() {
        let cables_at: HashSet<[i32; 3]> = [[0, 0, 0], [1, 0, 0], [2, 0, 0], [2, 1, 0], [9, 9, 9]].into();
        let all = cables(&[[0, 0, 0]], |p| cables_at.contains(&p), 100);
        assert_eq!(all.len(), 4);
        assert!(!all.contains(&[9, 9, 9]));
        assert_eq!(cables(&[[0, 0, 0]], |p| cables_at.contains(&p), 2).len(), 2);
        // A start that is not a cable gives no network.
        assert!(cables(&[[5, 5, 5]], |p| cables_at.contains(&p), 100).is_empty());
    }

    #[test]
    fn a_connector_hangs_on_any_side_but_the_one_it_faces() {
        // Faces east (5), a cable to its west is its back; one to the east is its target.
        assert!(attached([0, 0, 0], 5, [-1, 0, 0]));
        assert!(!attached([0, 0, 0], 5, [1, 0, 0]));
        assert!(attached([0, 0, 0], 5, [0, 1, 0]));
        assert!(!attached([0, 0, 0], 5, [3, 0, 0]));
    }

    #[test]
    fn extractors_feed_the_inserters_of_their_channel_by_priority() {
        let conns = [
            conn([0, 0, 0], 4, Mode::Extract, 3, 0),
            conn([5, 0, 0], 4, Mode::Insert, 3, 0),
            conn([2, 0, 0], 4, Mode::Insert, 3, 5),
            conn([1, 0, 0], 4, Mode::Insert, 4, 9),
            conn([7, 0, 0], 4, Mode::Off, 3, 9),
        ];
        let routes = plan(&conns);
        assert_eq!(routes, vec![Route { from: 0, to: vec![2, 1] }]);
    }

    #[test]
    fn a_connector_never_gives_back_into_what_it_takes_from() {
        // Two connectors facing the same chest from opposite cables.
        let a = conn([0, 0, 0], 5, Mode::Extract, 0, 0);
        let b = conn([2, 0, 0], 4, Mode::Insert, 0, 0);
        assert_eq!(a.target(), b.target());
        assert!(plan(&[a, b]).is_empty());
    }

    #[test]
    fn kinds_do_not_mix() {
        let mut fluid = conn([3, 0, 0], 4, Mode::Insert, 0, 0);
        fluid.kind = Kind::Fluids;
        assert!(plan(&[conn([0, 0, 0], 4, Mode::Extract, 0, 0), fluid]).is_empty());
    }
}
