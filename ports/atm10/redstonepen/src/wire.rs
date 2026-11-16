//! Redstone track networks: which track cells connect, and the power each one carries.
//! Pure logic, no host: the block part gathers the network and its inputs, this solves it.
use std::collections::HashMap;

use mod_api::Direction;

use crate::orient::{ALL, bit, step};

/// Power lost per wire block travelled.
pub const DECAY: u8 = 1;
/// The most wire blocks one update walks; a larger network is solved in parts.
pub const MAX_NODES: usize = 4096;

/// The positions a wire at `pos` with `faces` (bit mask, `orient::bit`) may connect to, each with
/// the faces a wire there needs to connect: straight neighbours always connect; a diagonal one
/// connects around the edge of the block both are attached to.
pub fn candidates(pos: [i32; 3], faces: u8) -> Vec<([i32; 3], u8)> {
    let mut out: Vec<([i32; 3], u8)> = ALL.iter().map(|d| (step(pos, *d), 0x3f)).collect();
    for a in ALL {
        if faces & bit(a) == 0 {
            continue;
        }
        // Attached on `a`: the support is at pos+a. Around each of its edges, a wire at
        // pos+a+b attached on -b sits on the same support.
        for b in ALL {
            if b == a || b == a.opposite() {
                continue;
            }
            out.push((step(step(pos, a), b), bit(b.opposite())));
        }
    }
    out
}

/// Whether a wire at `pos` (faces `faces`) connects to one at `other` (faces `other_faces`).
pub fn connects(pos: [i32; 3], faces: u8, other: [i32; 3], other_faces: u8) -> bool {
    let wraps = |p: [i32; 3], f: u8, q: [i32; 3], g: u8| {
        candidates(p, f)
            .into_iter()
            .any(|(c, need)| c == q && g & need != 0)
    };
    wraps(pos, faces, other, other_faces) || wraps(other, other_faces, pos, faces)
}

/// The power of every node: its own input, or the strongest connected node's less `DECAY`.
/// `inputs[i]` is node i's power from outside the network (0 to 15), `links[i]` its neighbours.
pub fn solve(inputs: &[u8], links: &[Vec<usize>]) -> Vec<u8> {
    let mut power: Vec<u8> = inputs.iter().map(|p| (*p).min(15)).collect();
    // Walk from the strongest down: each level fills the next one.
    let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); 16];
    for (i, p) in power.iter().enumerate() {
        buckets[usize::from(*p)].push(i);
    }
    for level in (1..=15u8).rev() {
        let nodes = std::mem::take(&mut buckets[usize::from(level)]);
        let next = level.saturating_sub(DECAY);
        for i in nodes {
            if power[i] != level {
                continue;
            }
            for &j in &links[i] {
                if power[j] < next {
                    power[j] = next;
                    buckets[usize::from(next)].push(j);
                }
            }
        }
    }
    power
}

/// A network gathered from the world: positions, faces and links.
#[derive(Debug, Default)]
pub struct Network {
    pub nodes: Vec<[i32; 3]>,
    pub faces: Vec<u8>,
    pub links: Vec<Vec<usize>>,
}

/// Gather the network from `start` by asking `faces_at` which wire faces a position holds
/// (`None` where there is no wire), up to `MAX_NODES`.
pub fn gather(start: [i32; 3], mut faces_at: impl FnMut([i32; 3]) -> Option<u8>) -> Network {
    let mut net = Network::default();
    let mut index: HashMap<[i32; 3], usize> = HashMap::new();
    let Some(f) = faces_at(start) else {
        return net;
    };
    index.insert(start, 0);
    net.nodes.push(start);
    net.faces.push(f);
    net.links.push(Vec::new());
    let mut i = 0;
    while i < net.nodes.len() {
        let (pos, faces) = (net.nodes[i], net.faces[i]);
        for (c, _) in candidates(pos, faces) {
            let j = match index.get(&c) {
                Some(j) => *j,
                None => {
                    if net.nodes.len() >= MAX_NODES {
                        continue;
                    }
                    let Some(g) = faces_at(c) else {
                        continue;
                    };
                    if !connects(pos, faces, c, g) {
                        continue;
                    }
                    let j = net.nodes.len();
                    index.insert(c, j);
                    net.nodes.push(c);
                    net.faces.push(g);
                    net.links.push(Vec::new());
                    j
                }
            };
            if j != i && connects(pos, faces, c, net.faces[j]) && !net.links[i].contains(&j) {
                net.links[i].push(j);
                net.links[j].push(i);
            }
        }
        i += 1;
    }
    net
}

/// The faces of the support blocks a wire is attached to, from its mask.
pub fn attached(faces: u8) -> impl Iterator<Item = Direction> {
    ALL.into_iter().filter(move |d| faces & bit(*d) != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Direction::*;

    #[test]
    fn power_falls_by_one_per_block_along_a_line() {
        let links = vec![vec![1], vec![0, 2], vec![1, 3], vec![2]];
        assert_eq!(solve(&[15, 0, 0, 0], &links), vec![15, 14, 13, 12]);
    }

    #[test]
    fn the_stronger_of_two_sources_wins_where_they_meet() {
        let links = vec![vec![1], vec![0, 2], vec![1, 3], vec![2, 4], vec![3]];
        assert_eq!(solve(&[15, 0, 0, 0, 12], &links), vec![15, 14, 13, 12, 12]);
    }

    #[test]
    fn a_loop_without_a_source_carries_nothing() {
        let links = vec![vec![1, 2], vec![0, 2], vec![0, 1]];
        assert_eq!(solve(&[0, 0, 0], &links), vec![0, 0, 0]);
    }

    #[test]
    fn a_floor_wire_wraps_down_the_side_of_its_support() {
        // On top of the block at (0,-1,0); a wire on that block's east side sits at (1,-1,0),
        // attached on west.
        assert!(connects([0, 0, 0], bit(Down), [1, -1, 0], bit(West)));
        // A wire there attached to something else does not share the edge.
        assert!(!connects([0, 0, 0], bit(Down), [1, -1, 0], bit(Down)));
    }

    #[test]
    fn floor_wires_one_block_apart_diagonally_do_not_connect() {
        assert!(!connects([0, 0, 0], bit(Down), [1, 0, 1], bit(Down)));
    }

    #[test]
    fn gathering_follows_only_connected_wires() {
        let wires: HashMap<[i32; 3], u8> = [
            ([0, 0, 0], bit(Down)),
            ([1, 0, 0], bit(Down)),
            ([2, -1, 0], bit(West) | bit(Down)),
            ([5, 0, 0], bit(Down)),
        ]
        .into();
        let net = gather([0, 0, 0], |p| wires.get(&p).copied());
        assert_eq!(net.nodes.len(), 3);
        assert!(!net.nodes.contains(&[5, 0, 0]));
    }
}
