//! The passive reactor's structure check and its numbers, apart from the game.
use std::collections::{HashSet, VecDeque};

/// What one cell of the world is to a reactor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Other,
    Casing,
    Controller,
    PowerTap,
    FuelRod,
    ControlRod,
}

/// Longest side of a reactor, in blocks.
pub const MAX_SIDE: i32 = 32;
/// Fuel units one ingot holds.
pub const UNITS_PER_INGOT: i32 = 1000;
/// Fuel units a fuel rod column holds.
pub const FUEL_PER_COLUMN: i32 = 4000;
/// Heat each fuel rod makes per tick at full activity.
pub const HEAT_PER_ROD: f64 = 2.0;
/// Fraction of the heat above ambient the casing sheds each tick.
pub const COOLING: f64 = 0.02;
/// Energy per unit of heat shed.
pub const FE_PER_HEAT: f64 = 10.0;
/// Ambient temperature.
pub const AMBIENT: f64 = 20.0;
/// Fuel units one rod burns per tick at full activity.
pub const FUEL_PER_ROD_TICK: f64 = 0.01;

/// A checked reactor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Structure {
    pub min: [i32; 3],
    pub max: [i32; 3],
    pub rods: u32,
    pub columns: u32,
    pub taps: Vec<[i32; 3]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    TooSmall,
    TooBig,
    NotAFrame,
    Shell,
    Interior,
    ControlRods,
    Controllers,
}

/// Finds the box of connected reactor parts around `controller` and checks it: a hollow cuboid at
/// least 3 on every side, casing on the edges, casing, the controller and power taps on the faces,
/// control rods on the top face above full columns of fuel rods, and nothing but fuel rods and
/// empty cells inside.
pub fn validate(controller: [i32; 3], get: impl Fn([i32; 3]) -> Part) -> Result<Structure, Fault> {
    let mut seen = HashSet::from([controller]);
    let mut queue = VecDeque::from([controller]);
    let (mut min, mut max) = (controller, controller);
    while let Some(p) = queue.pop_front() {
        for d in 0..3 {
            min[d] = min[d].min(p[d]);
            max[d] = max[d].max(p[d]);
        }
        if max.iter().zip(&min).any(|(a, b)| a - b + 1 > MAX_SIDE) || seen.len() > 40_000 {
            return Err(Fault::TooBig);
        }
        for d in 0..3 {
            for s in [-1, 1] {
                let mut n = p;
                n[d] += s;
                if get(n) != Part::Other && seen.insert(n) {
                    queue.push_back(n);
                }
            }
        }
    }
    if (0..3).any(|d| max[d] - min[d] < 2) {
        return Err(Fault::TooSmall);
    }
    let (mut controllers, mut rods, mut columns, mut taps) = (0, 0u32, 0u32, Vec::new());
    for x in min[0]..=max[0] {
        for y in min[1]..=max[1] {
            for z in min[2]..=max[2] {
                let p = [x, y, z];
                let part = get(p);
                let on = [x == min[0] || x == max[0], y == min[1] || y == max[1], z == min[2] || z == max[2]];
                let faces = on.iter().filter(|b| **b).count();
                match (faces, part) {
                    (0, Part::FuelRod) => rods += 1,
                    (0, Part::Other) => {}
                    (0, _) => return Err(Fault::Interior),
                    (2.., Part::Casing) => {}
                    (2.., _) => return Err(Fault::NotAFrame),
                    (_, Part::Casing) => {}
                    (_, Part::Controller) => controllers += 1,
                    (_, Part::PowerTap) => taps.push(p),
                    (_, Part::ControlRod) => {
                        // only on the top face, over a full column of fuel rods
                        if y != max[1] || !(min[1] + 1..max[1]).all(|yy| get([x, yy, z]) == Part::FuelRod) {
                            return Err(Fault::ControlRods);
                        }
                        columns += 1;
                    }
                    _ => return Err(Fault::Shell),
                }
            }
        }
    }
    if controllers != 1 {
        return Err(Fault::Controllers);
    }
    // every column of fuel rods needs its control rod, and no half columns
    for x in min[0] + 1..max[0] {
        for z in min[2] + 1..max[2] {
            let full = (min[1] + 1..max[1]).filter(|y| get([x, *y, z]) == Part::FuelRod).count();
            let height = (max[1] - min[1] - 1) as usize;
            let capped = get([x, max[1], z]) == Part::ControlRod;
            if (full == height) != capped || (full != 0 && full != height) {
                return Err(Fault::ControlRods);
            }
        }
    }
    Ok(Structure { min, max, rods, columns, taps })
}

/// The reactor's running state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Core {
    pub fuel: f64,
    pub waste: f64,
    pub heat: f64,
}

impl Default for Core {
    fn default() -> Self {
        Core { fuel: 0.0, waste: 0.0, heat: AMBIENT }
    }
}

impl Core {
    /// One tick with `rods` fuel rods, control rods inserted `insertion` percent: the energy shed.
    pub fn tick(&mut self, rods: u32, insertion: i32, running: bool) -> f64 {
        let activity = f64::from(100 - insertion.clamp(0, 100)) / 100.0;
        let mut gain = 0.0;
        if running && self.fuel > 0.0 {
            let burn = (f64::from(rods) * activity * FUEL_PER_ROD_TICK).min(self.fuel);
            self.fuel -= burn;
            self.waste += burn;
            // a rod short of fuel burns proportionally less
            gain = HEAT_PER_ROD * f64::from(rods) * activity * burn / (f64::from(rods) * activity * FUEL_PER_ROD_TICK).max(f64::MIN_POSITIVE);
        }
        let shed = (self.heat - AMBIENT) * COOLING;
        self.heat += gain - shed;
        shed * FE_PER_HEAT
    }

    /// Room for ingots in a reactor with `columns` fuel columns, in units.
    pub fn fuel_room(&self, columns: u32) -> f64 {
        f64::from(columns as i32 * FUEL_PER_COLUMN) - self.fuel - self.waste
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A 5x4x5 reactor: casing frame, controller on the front face, a power tap, 3x3 fuel columns
    /// of height 2 with control rods on top.
    fn build() -> HashMap<[i32; 3], Part> {
        let mut w = HashMap::new();
        for x in 0..5 {
            for y in 0..4 {
                for z in 0..5 {
                    let on = [x == 0 || x == 4, y == 0 || y == 3, z == 0 || z == 4];
                    let faces = on.iter().filter(|b| **b).count();
                    let part = match faces {
                        0 => Part::FuelRod,
                        1 if y == 3 => Part::ControlRod,
                        _ => Part::Casing,
                    };
                    w.insert([x, y, z], part);
                }
            }
        }
        w.insert([2, 1, 0], Part::Controller);
        w.insert([2, 2, 4], Part::PowerTap);
        w
    }

    fn check(w: &HashMap<[i32; 3], Part>) -> Result<Structure, Fault> {
        validate([2, 1, 0], |p| w.get(&p).copied().unwrap_or(Part::Other))
    }

    #[test]
    fn a_closed_box_with_capped_columns_is_a_reactor() {
        let s = check(&build()).unwrap();
        assert_eq!((s.rods, s.columns), (18, 9));
        assert_eq!(s.taps, vec![[2, 2, 4]]);
        assert_eq!((s.min, s.max), ([0, 0, 0], [4, 3, 4]));
    }

    #[test]
    fn a_hole_in_the_wall_a_second_controller_or_a_loose_control_rod_each_fail() {
        let mut w = build();
        w.insert([0, 1, 2], Part::Other);
        assert!(check(&w).is_err());
        let mut w = build();
        w.insert([4, 1, 2], Part::Controller);
        assert_eq!(check(&w), Err(Fault::Controllers));
        let mut w = build();
        w.insert([2, 2, 2], Part::Other); // a half column under its control rod
        assert_eq!(check(&w), Err(Fault::ControlRods));
    }

    #[test]
    fn fuel_burns_and_heat_settles_at_the_rate_the_rods_make() {
        let mut core = Core { fuel: 4000.0, ..Core::default() };
        let mut last = 0.0;
        for _ in 0..2000 {
            last = core.tick(18, 0, true);
        }
        // steady state: shed heat equals the heat made, 18 rods x 2.0, times 10 FE per heat
        assert!((last - 360.0).abs() < 1.0, "{last}");
        // 18 rods x 0.01 units x 2000 ticks, all of it waste
        assert!((core.fuel - (4000.0 - 360.0)).abs() < 1e-6);
        assert!((core.waste - 360.0).abs() < 1e-6);
    }

    #[test]
    fn inserted_control_rods_slow_the_reaction_and_a_stopped_reactor_cools() {
        let mut half = Core { fuel: 1000.0, ..Core::default() };
        let mut full = half;
        for _ in 0..1000 {
            half.tick(9, 50, true);
            full.tick(9, 0, true);
        }
        assert!((half.waste * 2.0 - full.waste).abs() < 1e-6);
        let hot = full.heat;
        full.tick(9, 0, false);
        assert!(full.heat < hot);
    }
}
