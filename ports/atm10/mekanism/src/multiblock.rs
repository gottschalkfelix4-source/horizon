//! Finding Mekanism's multiblocks: a hollow box of casing and ports (3 to 18 blocks along each
//! axis) with allowed blocks or air inside. Pure over closures, so the rule is tested without
//! the game.
use std::collections::{BTreeSet, VecDeque};

pub type Pos = [i32; 3];

/// A structure that was found.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Found {
    pub min: Pos,
    pub max: Pos,
    /// Every block of the shell, sorted.
    pub shell: Vec<Pos>,
    /// Every position inside, sorted.
    pub interior: Vec<Pos>,
}

pub const MIN_SIZE: i32 = 3;
pub const MAX_SIZE: i32 = 18;

/// The structure `start` (a shell block) belongs to: the shell blocks joined to it must be exactly
/// the surface of a box of a valid size, and everything inside must be `interior` (air counts).
pub fn find(
    start: Pos,
    is_shell: impl Fn(Pos) -> bool,
    is_interior: impl Fn(Pos) -> bool,
) -> Option<Found> {
    if !is_shell(start) {
        return None;
    }
    let mut seen = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(p) = queue.pop_front() {
        if seen.len() > (MAX_SIZE * MAX_SIZE * 6) as usize {
            return None;
        }
        for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
            let n = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
            if !seen.contains(&n) && is_shell(n) {
                seen.insert(n);
                queue.push_back(n);
            }
        }
    }
    let mut min = start;
    let mut max = start;
    for p in &seen {
        for i in 0..3 {
            min[i] = min[i].min(p[i]);
            max[i] = max[i].max(p[i]);
        }
    }
    if (0..3).any(|i| !(MIN_SIZE..=MAX_SIZE).contains(&(max[i] - min[i] + 1))) {
        return None;
    }
    let on_surface = |p: Pos| (0..3).any(|i| p[i] == min[i] || p[i] == max[i]);
    let mut interior = Vec::new();
    for x in min[0]..=max[0] {
        for y in min[1]..=max[1] {
            for z in min[2]..=max[2] {
                let p = [x, y, z];
                if on_surface(p) {
                    // the whole surface is shell (and the edges and corners too)
                    if !seen.contains(&p) {
                        return None;
                    }
                } else {
                    if seen.contains(&p) || !is_interior(p) {
                        return None;
                    }
                    interior.push(p);
                }
            }
        }
    }
    Some(Found { min, max, shell: seen.into_iter().collect(), interior })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A 3x3x3 box of 'C' (casing) around one 'I' (inside), changed by `edit`.
    fn world(edit: &[(Pos, char)]) -> HashMap<Pos, char> {
        let mut w = HashMap::new();
        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    let inside = [x, y, z] == [1, 1, 1];
                    w.insert([x, y, z], if inside { 'I' } else { 'C' });
                }
            }
        }
        for (p, c) in edit {
            w.insert(*p, *c);
        }
        w
    }

    fn run(w: &HashMap<Pos, char>) -> Option<Found> {
        find([0, 0, 0], |p| w.get(&p) == Some(&'C'), |p| matches!(w.get(&p), Some('I') | None))
    }

    #[test]
    fn a_closed_box_is_found_with_its_inside() {
        let f = run(&world(&[])).unwrap();
        assert_eq!((f.min, f.max), ([0, 0, 0], [2, 2, 2]));
        assert_eq!(f.shell.len(), 26);
        assert_eq!(f.interior, vec![[1, 1, 1]]);
    }

    #[test]
    fn a_hole_in_the_shell_or_a_foreign_block_inside_breaks_it() {
        assert_eq!(run(&world(&[([1, 0, 1], 'X')])), None);
        assert_eq!(run(&world(&[([1, 1, 1], 'X')])), None);
    }

    #[test]
    fn a_stray_block_on_the_shell_and_a_box_too_small_do_not_count() {
        // a casing block stuck on the outside is joined to the shell but not on its surface
        assert_eq!(run(&world(&[([3, 1, 1], 'C')])), None);
        let mut small = HashMap::new();
        small.insert([0, 0, 0], 'C');
        assert_eq!(run(&small), None);
    }

    #[test]
    fn air_inside_is_fine() {
        let mut w = world(&[]);
        w.remove(&[1, 1, 1]);
        assert!(run(&w).is_some());
    }
}
