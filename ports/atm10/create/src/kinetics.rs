//! Create's rotation and stress, free of the game so it can be tested.
//!
//! A kinetic network is the set of blocks joined by shafts, cogwheels and gearboxes. One speed
//! reaches every block of it (scaled by the cogwheels between), sources add stress capacity,
//! consumers use stress. All numbers are from the default config as remembered and are listed in
//! README.md under "To check".
use std::collections::{BTreeMap, HashMap, VecDeque};

pub type Pos = [i32; 3];

/// The most blocks one network holds; a larger one is cut off.
pub const MAX_NETWORK: usize = 4096;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub fn index(self) -> usize {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        }
    }

    pub fn from_name(name: &str) -> Option<Axis> {
        match name {
            "x" => Some(Axis::X),
            "y" => Some(Axis::Y),
            "z" => Some(Axis::Z),
            _ => None,
        }
    }

    /// The axis a unit offset runs along, if it is one.
    pub fn of_offset(d: [i32; 3]) -> Option<Axis> {
        match d {
            [_, 0, 0] if d[0] != 0 => Some(Axis::X),
            [0, _, 0] if d[1] != 0 => Some(Axis::Y),
            [0, 0, _] if d[2] != 0 => Some(Axis::Z),
            _ => None,
        }
    }
}

/// What a block does in a network.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Part {
    /// Passes rotation along its axis.
    Shaft,
    /// Passes it along its axis and meshes with cogwheels beside it.
    Cog,
    /// A large cogwheel: meshes with small ones beside it and large ones diagonally.
    LargeCog,
    /// Joins shafts on all six faces.
    Gearbox,
    /// Makes rotation: `rpm` (sign: the sense, relative to the block's positive axis) and the
    /// stress it supports per RPM.
    Source { rpm: f64, capacity: f64 },
    /// Uses `impact` stress per RPM.
    Consumer { impact: f64 },
    /// A belt block: a consumer that also passes the rotation to the belt blocks in line with it
    /// (across its axis, in the horizontal plane).
    Belt { impact: f64 },
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Node {
    pub part: Part,
    pub axis: Axis,
}

impl Node {
    pub fn new(part: Part, axis: Axis) -> Node {
        Node { part, axis }
    }

    fn is_cog(&self) -> bool {
        matches!(self.part, Part::Cog | Part::LargeCog)
    }

    /// Whether the block's axle reaches the face towards `d`.
    fn axle_faces(&self, d: [i32; 3]) -> bool {
        matches!(self.part, Part::Gearbox) || Axis::of_offset(d) == Some(self.axis)
    }
}

/// How `from`'s rotation reaches `to` one block away at `d` (offset from `from`):
/// `(speed factor to / from, sign flip)`, or `None` when the two do not touch.
pub fn link(from: &Node, to: &Node, d: [i32; 3]) -> Option<(f64, bool)> {
    // axle to axle: both face each other
    let back = [-d[0], -d[1], -d[2]];
    if from.axle_faces(d) && to.axle_faces(back) && Axis::of_offset(d).is_some() {
        // a gearbox senses its Y faces the other way, so a turn between X and Y faces (or Y and Z)
        // reverses the rotation and a straight line through it does not
        let a = Axis::of_offset(d).unwrap_or(Axis::X);
        let reverse = |n: &Node| matches!(n.part, Part::Gearbox) && a == Axis::Y;
        let turns = reverse(from) != reverse(to);
        return Some((1.0, turns));
    }
    // belt blocks in a row turn together
    if let (Part::Belt { .. }, Part::Belt { .. }) = (from.part, to.part) {
        if from.axis == to.axis && d[1] == 0 && d[from.axis.index()] == 0 {
            return Some((1.0, false));
        }
        return None;
    }
    // cogwheel teeth: parallel axes, offset in the plane they turn in
    if from.is_cog() && to.is_cog() && from.axis == to.axis && d[from.axis.index()] == 0 {
        let flat = d.iter().filter(|c| **c != 0).count();
        let big_from = matches!(from.part, Part::LargeCog);
        let big_to = matches!(to.part, Part::LargeCog);
        return match (big_from, big_to, flat) {
            (false, false, 1) => Some((1.0, true)),
            (true, true, 2) => Some((1.0, true)),
            // a small cog turns twice as fast as the large one it meshes with
            (true, false, 1) => Some((2.0, true)),
            (false, true, 1) => Some((0.5, true)),
            _ => None,
        };
    }
    None
}

/// Every position a network reaches from `start`, in discovery order.
fn neighbours(node: &Node) -> Vec<[i32; 3]> {
    let mut out = Vec::new();
    for d in [
        [1, 0, 0],
        [-1, 0, 0],
        [0, 1, 0],
        [0, -1, 0],
        [0, 0, 1],
        [0, 0, -1],
    ] {
        out.push(d);
    }
    if node.is_cog() {
        for a in [-1, 0, 1] {
            for b in [-1, 0, 1] {
                for c in [-1, 0, 1] {
                    let d = [a, b, c];
                    if d.iter().filter(|x| **x != 0).count() == 2 && d[node.axis.index()] == 0 {
                        out.push(d);
                    }
                }
            }
        }
    }
    out
}

/// What a solved network says about its blocks.
#[derive(Clone, Debug, Default)]
pub struct Solution {
    /// Each block's signed speed in RPM (0 when the network is jammed or has no source).
    pub speeds: BTreeMap<Pos, f64>,
    /// Stress the sources support, in su (capacity per RPM times RPM).
    pub capacity: f64,
    /// Stress the consumers use.
    pub stress: f64,
    /// Sources or cogwheels that cannot agree (different speeds or a closed loop that turns
    /// against itself): nothing in the network turns.
    pub conflict: bool,
    /// The consumers want more than the sources give: nothing turns either.
    pub overstressed: bool,
}

impl Solution {
    pub fn leader(&self) -> Option<Pos> {
        self.speeds.keys().next().copied()
    }
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// Solves the network that contains `start`. `node_at` says what the block at a position is.
pub fn solve(node_at: impl Fn(Pos) -> Option<Node>, start: Pos) -> Solution {
    let mut nodes: HashMap<Pos, Node> = HashMap::new();
    // relative speed (signed) of each block when the network's first block turns at 1
    let mut rel: HashMap<Pos, f64> = HashMap::new();
    let mut out = Solution::default();
    let Some(first) = node_at(start) else {
        return out;
    };
    nodes.insert(start, first);
    rel.insert(start, 1.0);
    let mut queue = VecDeque::from([start]);
    while let Some(at) = queue.pop_front() {
        let node = nodes[&at];
        let here = rel[&at];
        for d in neighbours(&node) {
            let to = [at[0] + d[0], at[1] + d[1], at[2] + d[2]];
            let other = match nodes.get(&to) {
                Some(n) => *n,
                None => {
                    let Some(n) = node_at(to) else { continue };
                    n
                }
            };
            let Some((factor, flip)) = link(&node, &other, d) else {
                continue;
            };
            let there = here * factor * if flip { -1.0 } else { 1.0 };
            match rel.get(&to) {
                Some(known) => {
                    if !near(*known, there) {
                        out.conflict = true;
                    }
                }
                None => {
                    if nodes.len() >= MAX_NETWORK {
                        continue;
                    }
                    nodes.insert(to, other);
                    rel.insert(to, there);
                    queue.push_back(to);
                }
            }
        }
    }
    // the sources must all agree on the speed the first block turns at
    let mut root: Option<f64> = None;
    for (pos, node) in &nodes {
        if let Part::Source { rpm, .. } = node.part {
            let v = rpm / rel[pos];
            match root {
                None => root = Some(v),
                Some(r) if !near(r, v) => out.conflict = true,
                _ => {}
            }
        }
    }
    let root = if out.conflict {
        0.0
    } else {
        root.unwrap_or(0.0)
    };
    for (pos, node) in &nodes {
        let speed = rel[pos] * root;
        match node.part {
            Part::Source { capacity, .. } => out.capacity += capacity * speed.abs(),
            Part::Consumer { impact } | Part::Belt { impact } => out.stress += impact * speed.abs(),
            _ => {}
        }
    }
    out.overstressed = out.stress > out.capacity + 1e-6;
    let run = !out.conflict && !out.overstressed;
    for pos in nodes.keys() {
        out.speeds
            .insert(*pos, if run { rel[pos] * root } else { 0.0 });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(blocks: &[(Pos, Node)]) -> impl Fn(Pos) -> Option<Node> + use<> {
        let map: HashMap<Pos, Node> = blocks.iter().copied().collect();
        move |p| map.get(&p).copied()
    }

    fn shaft(x: i32) -> (Pos, Node) {
        ([x, 0, 0], Node::new(Part::Shaft, Axis::X))
    }

    fn crank(x: i32, rpm: f64) -> (Pos, Node) {
        (
            [x, 0, 0],
            Node::new(
                Part::Source {
                    rpm,
                    capacity: 32.0,
                },
                Axis::X,
            ),
        )
    }

    fn load(x: i32, impact: f64) -> (Pos, Node) {
        ([x, 0, 0], Node::new(Part::Consumer { impact }, Axis::X))
    }

    #[test]
    fn a_shaft_line_carries_one_speed() {
        let w = world(&[crank(0, 16.0), shaft(1), shaft(2), shaft(3)]);
        let s = solve(&w, [3, 0, 0]);
        assert_eq!(s.speeds.len(), 4);
        assert!(s.speeds.values().all(|v| near(*v, 16.0)));
        assert!(near(s.capacity, 512.0));
    }

    #[test]
    fn a_shaft_beside_the_line_is_not_joined() {
        let w = world(&[
            crank(0, 16.0),
            shaft(1),
            ([2, 1, 0], Node::new(Part::Shaft, Axis::X)),
        ]);
        assert_eq!(solve(&w, [0, 0, 0]).speeds.len(), 2);
    }

    #[test]
    fn a_shaft_across_the_line_does_not_join_an_axle_end() {
        let w = world(&[crank(0, 16.0), ([1, 0, 0], Node::new(Part::Shaft, Axis::Y))]);
        assert_eq!(solve(&w, [0, 0, 0]).speeds.len(), 1);
    }

    #[test]
    fn two_small_cogs_side_by_side_turn_the_other_way() {
        let c = |p: Pos| (p, Node::new(Part::Cog, Axis::Y));
        let w = world(&[
            (
                [0, 0, 0],
                Node::new(
                    Part::Source {
                        rpm: 8.0,
                        capacity: 32.0,
                    },
                    Axis::Y,
                ),
            ),
            c([0, 1, 0]),
            c([1, 1, 0]),
            c([2, 1, 0]),
        ]);
        let s = solve(&w, [0, 0, 0]);
        assert!(near(s.speeds[&[0, 1, 0]], 8.0));
        assert!(near(s.speeds[&[1, 1, 0]], -8.0));
        assert!(near(s.speeds[&[2, 1, 0]], 8.0));
    }

    #[test]
    fn a_small_cog_on_a_large_one_turns_twice_as_fast() {
        let w = world(&[
            (
                [0, 0, 0],
                Node::new(
                    Part::Source {
                        rpm: 8.0,
                        capacity: 32.0,
                    },
                    Axis::Y,
                ),
            ),
            ([0, 1, 0], Node::new(Part::LargeCog, Axis::Y)),
            ([1, 1, 0], Node::new(Part::Cog, Axis::Y)),
        ]);
        let s = solve(&w, [0, 0, 0]);
        assert!(near(s.speeds[&[1, 1, 0]], -16.0));
    }

    #[test]
    fn large_cogs_mesh_diagonally_not_side_by_side() {
        let big = |p: Pos| (p, Node::new(Part::LargeCog, Axis::Y));
        let diagonal = world(&[big([0, 0, 0]), big([1, 0, 1])]);
        assert_eq!(solve(&diagonal, [0, 0, 0]).speeds.len(), 2);
        let beside = world(&[big([0, 0, 0]), big([1, 0, 0])]);
        assert_eq!(solve(&beside, [0, 0, 0]).speeds.len(), 1);
    }

    #[test]
    fn a_gearbox_turns_a_corner_and_reverses() {
        let w = world(&[
            crank(0, 16.0),
            ([1, 0, 0], Node::new(Part::Gearbox, Axis::X)),
            ([1, 1, 0], Node::new(Part::Shaft, Axis::Y)),
            shaft(2),
        ]);
        let s = solve(&w, [0, 0, 0]);
        assert!(near(s.speeds[&[1, 1, 0]], -16.0));
        assert!(near(s.speeds[&[2, 0, 0]], 16.0));
    }

    #[test]
    fn sources_that_disagree_jam_the_network() {
        let w = world(&[crank(0, 16.0), shaft(1), crank(2, 32.0)]);
        let s = solve(&w, [1, 0, 0]);
        assert!(s.conflict);
        assert!(s.speeds.values().all(|v| *v == 0.0));
    }

    #[test]
    fn sources_that_agree_add_their_capacity() {
        let w = world(&[crank(0, 16.0), shaft(1), crank(2, 16.0)]);
        let s = solve(&w, [1, 0, 0]);
        assert!(!s.conflict);
        assert!(near(s.capacity, 1024.0));
    }

    #[test]
    fn a_ring_that_turns_against_itself_jams() {
        // a small cog beside two large ones that also touch each other diagonally: the two paths
        // to the second large cog give opposite senses
        let w = world(&[
            (
                [0, 0, 0],
                Node::new(
                    Part::Source {
                        rpm: 8.0,
                        capacity: 32.0,
                    },
                    Axis::Y,
                ),
            ),
            ([0, 1, 0], Node::new(Part::LargeCog, Axis::Y)),
            ([1, 1, 0], Node::new(Part::Cog, Axis::Y)),
            ([1, 1, 1], Node::new(Part::LargeCog, Axis::Y)),
        ]);
        let s = solve(&w, [0, 0, 0]);
        assert!(s.conflict);
        assert!(s.speeds.values().all(|v| *v == 0.0));
    }

    #[test]
    fn consumers_use_stress_by_their_own_speed() {
        // 8 rpm source, impact 8 per rpm: 64 used of 256 supported
        let w = world(&[crank(0, 8.0), load(1, 8.0)]);
        let s = solve(&w, [0, 0, 0]);
        assert!(near(s.stress, 64.0));
        assert!(near(s.capacity, 256.0));
        assert!(!s.overstressed);
    }

    #[test]
    fn too_many_consumers_stop_the_network() {
        let w = world(&[
            crank(0, 8.0),
            load(1, 8.0),
            load(2, 8.0),
            load(3, 8.0),
            load(4, 8.0),
            load(5, 8.0),
        ]);
        let s = solve(&w, [0, 0, 0]);
        assert!(s.overstressed);
        assert!(s.speeds.values().all(|v| *v == 0.0));
    }

    #[test]
    fn without_a_source_nothing_turns() {
        let w = world(&[shaft(0), shaft(1)]);
        let s = solve(&w, [0, 0, 0]);
        assert_eq!(s.speeds.len(), 2);
        assert!(s.speeds.values().all(|v| *v == 0.0));
    }

    #[test]
    fn the_leader_is_the_lowest_position() {
        let w = world(&[crank(0, 8.0), shaft(1), shaft(-1)]);
        assert_eq!(solve(&w, [1, 0, 0]).leader(), Some([-1, 0, 0]));
    }

    #[test]
    fn belt_blocks_in_a_row_turn_together() {
        let belt = |p: Pos| (p, Node::new(Part::Belt { impact: 0.0 }, Axis::X));
        let w = world(&[
            (
                [-1, 0, 0],
                Node::new(
                    Part::Source {
                        rpm: 16.0,
                        capacity: 32.0,
                    },
                    Axis::X,
                ),
            ),
            belt([0, 0, 0]),
            belt([0, 0, 1]),
            belt([0, 0, 2]),
            belt([1, 0, 3]),
        ]);
        let s = solve(&w, [0, 0, 2]);
        assert!(near(s.speeds[&[0, 0, 0]], 16.0));
        assert!(near(s.speeds[&[0, 0, 2]], 16.0));
        // not in line: stays out
        assert!(!s.speeds.contains_key(&[1, 0, 3]));
    }
}
