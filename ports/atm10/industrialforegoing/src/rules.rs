//! Industrial Foregoing's rules that need no game: addon effects, working areas, tanks, the
//! machines' numbers and recipes. The glue in `lib.rs` reads the world and carries them out.
//!
//! Every number here is the port's reading of the mod's public behaviour and default config;
//! README.md lists the ones still to confirm against the NeoForge original.
use mod_api::{Compound, Tag};

/// The key, inside an item's `minecraft:custom_data`, that makes it an addon:
/// `{"industrialforegoing:addon": {kind: "speed", tier: 1}}`. More Industrial Foregoing Addons
/// (the `mifa` port) gives its items the same component, so the two ports need no code from
/// each other.
pub const ADDON_KEY: &str = "industrialforegoing:addon";

/// One addon kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddonKind {
    /// Shortens a machine's cycle.
    Speed,
    /// Lowers the energy one operation costs.
    Efficiency,
    /// Lets one cycle process more than one operation.
    Processing,
    /// Widens a machine's working area.
    Range,
}

impl AddonKind {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "speed" => Some(Self::Speed),
            "efficiency" => Some(Self::Efficiency),
            "processing" => Some(Self::Processing),
            "range" => Some(Self::Range),
            _ => None,
        }
    }
}

/// The addons in a machine's addon slots, the best tier of each kind (IF does not stack two
/// addons of a kind: a machine takes one of each).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Addons {
    pub speed: u8,
    pub efficiency: u8,
    pub processing: u8,
    pub range: u8,
}

impl Addons {
    /// Adds the addon described by an item's custom data, if it is one.
    pub fn add_custom_data(&mut self, custom_data: &Compound) {
        let Some(Tag::Compound(addon)) = custom_data.get(&ADDON_KEY.into()) else {
            return;
        };
        let kind = match addon.get(&"kind".into()) {
            Some(Tag::String(s)) => AddonKind::parse(&s.to_string_lossy()),
            _ => None,
        };
        let tier = match addon.get(&"tier".into()) {
            Some(Tag::Int(t)) => (*t).clamp(0, 255) as u8,
            Some(Tag::Byte(t)) => (*t).max(0) as u8,
            Some(Tag::Short(t)) => (*t).clamp(0, 255) as u8,
            _ => 0,
        };
        let slot = match kind {
            Some(AddonKind::Speed) => &mut self.speed,
            Some(AddonKind::Efficiency) => &mut self.efficiency,
            Some(AddonKind::Processing) => &mut self.processing,
            Some(AddonKind::Range) => &mut self.range,
            None => return,
        };
        *slot = (*slot).max(tier);
    }

    /// Progress a cycle gains per tick: 1, plus one per speed tier.
    pub fn progress_per_tick(&self) -> i32 {
        1 + i32::from(self.speed)
    }

    /// Energy one operation costs: the base, less a tenth per efficiency tier, never under a
    /// tenth of the base (and never 0 for a base above 0).
    pub fn energy_per_operation(&self, base: i32) -> i32 {
        if base <= 0 {
            return 0;
        }
        let tenths = (10 - i32::from(self.efficiency)).max(1);
        ((i64::from(base) * i64::from(tenths)) / 10).max(1) as i32
    }

    /// Operations one finished cycle may run: 1, plus one per processing tier.
    pub fn operations_per_cycle(&self) -> u32 {
        1 + u32::from(self.processing)
    }

    /// The working area's horizontal radius: the machine's base radius plus the range tier.
    pub fn radius(&self, base: i32) -> i32 {
        base + i32::from(self.range)
    }
}

/// A horizontal facing, as the block's `facing` property names it.
pub fn facing_normal(facing: &str) -> Option<[i32; 3]> {
    match facing {
        "north" => Some([0, 0, -1]),
        "south" => Some([0, 0, 1]),
        "west" => Some([-1, 0, 0]),
        "east" => Some([1, 0, 0]),
        "up" => Some([0, 1, 0]),
        "down" => Some([0, -1, 0]),
        _ => None,
    }
}

/// A machine's working area: a box behind the machine (the side its front faces away from),
/// `radius` blocks to each side of the centre line, `height` blocks tall from the machine's
/// own y plus `y_offset`. Inclusive corners, min then max.
pub fn working_area(pos: [i32; 3], facing: [i32; 3], radius: i32, height: i32, y_offset: i32) -> ([i32; 3], [i32; 3]) {
    let back = [-facing[0], -facing[1], -facing[2]];
    let centre = [
        pos[0] + back[0] * (radius + 1),
        pos[1] + y_offset,
        pos[2] + back[2] * (radius + 1),
    ];
    (
        [centre[0] - radius, centre[1], centre[2] - radius],
        [centre[0] + radius, centre[1] + height - 1, centre[2] + radius],
    )
}

/// The `index`th position of an area, walking x, then z, then y (a scan the machine resumes
/// each operation through its saved cursor). Wraps around.
pub fn area_position(min: [i32; 3], max: [i32; 3], index: u32) -> [i32; 3] {
    let w = (max[0] - min[0] + 1).max(1) as u32;
    let d = (max[2] - min[2] + 1).max(1) as u32;
    let h = (max[1] - min[1] + 1).max(1) as u32;
    let i = index % (w * d * h);
    [
        min[0] + (i % w) as i32,
        min[1] + (i / (w * d)) as i32,
        min[2] + ((i / w) % d) as i32,
    ]
}

/// The number of positions in an area.
pub fn area_size(min: [i32; 3], max: [i32; 3]) -> u32 {
    (0..3).map(|i| (max[i] - min[i] + 1).max(1) as u32).product()
}

/// One tank of a machine, as the game saves it under `fluids`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tank {
    pub fluid: Option<String>,
    pub amount: i32,
}

impl Tank {
    pub fn holds(&self, fluid: &str) -> bool {
        self.amount > 0 && self.fluid.as_deref() == Some(fluid)
    }

    /// Adds up to `amount` of `fluid` within `capacity`; returns what went in.
    pub fn fill(&mut self, fluid: &str, amount: i32, capacity: i32) -> i32 {
        if self.amount > 0 && self.fluid.as_deref() != Some(fluid) {
            return 0;
        }
        let went = amount.min(capacity - self.amount).max(0);
        if went > 0 {
            self.fluid = Some(fluid.to_owned());
            self.amount += went;
        }
        went
    }

    /// Takes up to `amount`; returns what came out.
    pub fn drain(&mut self, amount: i32) -> i32 {
        let took = amount.min(self.amount).max(0);
        self.amount -= took;
        if self.amount == 0 {
            self.fluid = None;
        }
        took
    }
}

/// The tanks in a block entity's compound (`fluids: [{Fluid, Amount}]`), `n` long.
pub fn read_tanks(nbt: &Compound, n: usize) -> Vec<Tank> {
    let mut tanks: Vec<Tank> = match nbt.get(&"fluids".into()) {
        Some(Tag::List(list)) => list
            .iter()
            .take(n)
            .map(|tag| match tag {
                Tag::Compound(map) => Tank {
                    fluid: match map.get(&"Fluid".into()) {
                        Some(Tag::String(f)) if f.to_string_lossy() != "minecraft:empty" => {
                            Some(f.to_string_lossy())
                        }
                        _ => None,
                    },
                    amount: match map.get(&"Amount".into()) {
                        Some(Tag::Int(a)) => (*a).max(0),
                        _ => 0,
                    },
                },
                _ => Tank::default(),
            })
            .collect(),
        _ => Vec::new(),
    };
    tanks.resize(n, Tank::default());
    tanks
}

/// `read_tanks`'s writer.
pub fn write_tanks(nbt: &mut Compound, tanks: &[Tank]) {
    let list = tanks
        .iter()
        .map(|t| {
            let mut map = Compound::new();
            let fluid = if t.amount > 0 { t.fluid.as_deref().unwrap_or("minecraft:empty") } else { "minecraft:empty" };
            map.insert("Fluid".into(), Tag::String(fluid.into()));
            map.insert("Amount".into(), Tag::Int(t.amount.max(0)));
            Tag::Compound(map)
        })
        .collect();
    nbt.insert("fluids".into(), Tag::List(list));
}

/// An int field of a compound, 0 when missing.
pub fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

pub fn set_int(nbt: &mut Compound, key: &str, value: i32) {
    nbt.insert(key.into(), Tag::Int(value));
}

/// A long field of a compound, 0 when missing.
pub fn long(nbt: &Compound, key: &str) -> i64 {
    match nbt.get(&key.into()) {
        Some(Tag::Long(v)) => *v,
        Some(Tag::Int(v)) => i64::from(*v),
        _ => 0,
    }
}

/// A machine's cycle: progress grows each tick it has the energy for one operation, and a
/// full bar runs the operation. Returns how many cycles completed this tick (0 or 1).
///
/// `can_work` is whether the operation would do anything (input there, output room); a machine
/// with nothing to do keeps its progress and spends nothing, as IF's machines idle.
pub fn cycle(progress: &mut i32, max_progress: i32, step: i32, energy: i32, cost: i32, can_work: bool) -> bool {
    if !can_work || energy < cost {
        return false;
    }
    *progress += step;
    if *progress >= max_progress {
        *progress = 0;
        true
    } else {
        false
    }
}

/// The numbers of one machine kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MachineSpec {
    /// The block's path in the `industrialforegoing` namespace.
    pub name: &'static str,
    /// Ticks of progress one cycle needs.
    pub max_progress: i32,
    /// FE one operation costs (before efficiency addons).
    pub energy_per_operation: i32,
    /// The energy buffer.
    pub energy_capacity: i32,
    /// Working area: base radius (0 for no area), height, y offset from the machine.
    pub radius: i32,
    pub height: i32,
    pub y_offset: i32,
}

pub const FLUID_EXTRACTOR: MachineSpec = MachineSpec { name: "fluid_extractor", max_progress: 5, energy_per_operation: 10, energy_capacity: 10_000, radius: 0, height: 0, y_offset: 0 };
pub const LATEX_PROCESSING_UNIT: MachineSpec = MachineSpec { name: "latex_processing_unit", max_progress: 60, energy_per_operation: 40, energy_capacity: 10_000, radius: 0, height: 0, y_offset: 0 };
pub const DISSOLUTION_CHAMBER: MachineSpec = MachineSpec { name: "dissolution_chamber", max_progress: 1, energy_per_operation: 40, energy_capacity: 10_000, radius: 0, height: 0, y_offset: 0 };
pub const MOB_CRUSHER: MachineSpec = MachineSpec { name: "mob_crusher", max_progress: 60, energy_per_operation: 200, energy_capacity: 10_000, radius: 1, height: 3, y_offset: 0 };
pub const PLANT_GATHERER: MachineSpec = MachineSpec { name: "plant_gatherer", max_progress: 10, energy_per_operation: 50, energy_capacity: 10_000, radius: 1, height: 1, y_offset: 0 };
pub const PLANT_SOWER: MachineSpec = MachineSpec { name: "plant_sower", max_progress: 10, energy_per_operation: 50, energy_capacity: 10_000, radius: 1, height: 1, y_offset: 0 };

/// Every block entity machine, in registration order.
pub const MACHINES: [MachineSpec; 6] = [FLUID_EXTRACTOR, LATEX_PROCESSING_UNIT, DISSOLUTION_CHAMBER, MOB_CRUSHER, PLANT_GATHERER, PLANT_SOWER];

/// Tank size of the processing machines, mB.
pub const MACHINE_TANK: i32 = 8_000;

/// Latex the Fluid Extractor draws from the log it faces in one operation, mB.
pub const LATEX_PER_OPERATION: i32 = 3;
/// The chance, per operation, that the log is used up (turns to air).
pub const LOG_BREAK_CHANCE: f64 = 0.01;

/// Latex Processing Unit: latex and water for one Tiny Dry Rubber.
pub const LATEX_PER_RUBBER: i32 = 750;
pub const WATER_PER_RUBBER: i32 = 500;

/// Sludge the Plant Gatherer makes per plant harvested, mB.
pub const SLUDGE_PER_HARVEST: i32 = 10;

/// A Dissolution Chamber recipe: up to eight item inputs (ids, each one item, in any slot),
/// an input fluid, its processing time, and an item and/or fluid output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DissolutionRecipe {
    pub inputs: &'static [&'static str],
    pub fluid: (&'static str, i32),
    pub time: i32,
    pub output: Option<(&'static str, u32)>,
    pub output_fluid: Option<(&'static str, i32)>,
}

/// The chamber's recipes the port carries (IF ships more as data; see README).
pub const DISSOLUTION_RECIPES: &[DissolutionRecipe] = &[
    // Pity machine frame: wood and iron with latex.
    DissolutionRecipe {
        inputs: &["minecraft:oak_planks", "minecraft:oak_planks", "minecraft:oak_planks", "minecraft:oak_planks", "minecraft:iron_ingot", "minecraft:iron_ingot", "minecraft:iron_ingot", "minecraft:redstone"],
        fluid: ("industrialforegoing:latex", 250),
        time: 300,
        output: Some(("industrialforegoing:machine_frame_pity", 1)),
        output_fluid: None,
    },
    // Pink slime ingot: an iron and a gold ingot in pink slime.
    DissolutionRecipe {
        inputs: &["minecraft:iron_ingot", "minecraft:gold_ingot"],
        fluid: ("industrialforegoing:pink_slime", 1000),
        time: 300,
        output: Some(("industrialforegoing:pink_slime_ingot", 1)),
        output_fluid: None,
    },
    // Rubber from a slimeball and latex.
    DissolutionRecipe {
        inputs: &["minecraft:slime_ball"],
        fluid: ("industrialforegoing:latex", 250),
        time: 100,
        output: Some(("industrialforegoing:dryrubber", 1)),
        output_fluid: None,
    },
];

/// The recipe the chamber's items and fluid make, if any. `items` are the input slots' item ids
/// (one item of each is used); extra items in slots no ingredient wants stop the match, as a
/// shapeless recipe needs exactly its ingredients.
pub fn dissolution_match<'a>(items: &[Option<String>], tank: &Tank, recipes: &'a [DissolutionRecipe]) -> Option<&'a DissolutionRecipe> {
    recipes.iter().find(|r| {
        if !tank.holds(r.fluid.0) || tank.amount < r.fluid.1 {
            return false;
        }
        let mut have: Vec<&str> = items.iter().flatten().map(String::as_str).collect();
        if have.len() != r.inputs.len() {
            return false;
        }
        for want in r.inputs {
            match have.iter().position(|h| h == want) {
                Some(i) => {
                    have.swap_remove(i);
                }
                None => return false,
            }
        }
        true
    })
}

/// Black Hole Unit and Black Hole Tank tiers: (name prefix, items, millibuckets).
pub const BLACK_HOLE_TIERS: [(&str, i64, i32); 4] = [
    ("pity", 32_768, 64_000),
    ("simple", 524_288, 1_024_000),
    ("advanced", 4_194_304, 16_384_000),
    ("supreme", i32::MAX as i64, i32::MAX),
];

/// What a Black Hole Unit takes from its input: `stored` of the same item already, `offered`
/// in the slot, `capacity` in all. Returns how many go in.
pub fn black_hole_accepts(stored: i64, offered: u32, capacity: i64) -> u32 {
    (capacity - stored).clamp(0, i64::from(offered)) as u32
}

/// Generators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratorSpec {
    pub name: &'static str,
    /// FE made each tick it burns.
    pub per_tick: i32,
    pub capacity: i32,
    /// FE it pushes to its neighbours each tick, at most.
    pub push: i32,
}

/// The Pitiful Generator burns any furnace fuel for its vanilla burn time.
pub const PITIFUL_GENERATOR: GeneratorSpec = GeneratorSpec { name: "pitiful_generator", per_tick: 30, capacity: 100_000, push: 1_000 };
/// The Biofuel Generator burns biofuel from its tank.
pub const BIOFUEL_GENERATOR: GeneratorSpec = GeneratorSpec { name: "biofuel_generator", per_tick: 160, capacity: 1_000_000, push: 2_000 };
/// Biofuel burned per tick, mB.
pub const BIOFUEL_PER_TICK: i32 = 4;

/// One tick of a fuel generator: burns a tick of its fuel when there is room for its output.
/// `burn` is the ticks left of the current fuel; `next_fuel` the burn time of fuel it could
/// take now. Returns (FE made, whether it took the next fuel).
pub fn generator_tick(burn: &mut i32, energy: i32, spec: &GeneratorSpec, next_fuel: Option<i32>) -> (i32, bool) {
    if energy + spec.per_tick > spec.capacity {
        return (0, false);
    }
    let mut took = false;
    if *burn <= 0 {
        match next_fuel {
            Some(t) if t > 0 => {
                *burn = t;
                took = true;
            }
            _ => return (0, false),
        }
    }
    *burn -= 1;
    (spec.per_tick, took)
}

/// Blocks the Mob Crusher never kills.
pub fn crusher_spares(kind: &str) -> bool {
    matches!(kind, "minecraft:player" | "minecraft:wither" | "minecraft:ender_dragon" | "minecraft:armor_stand")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addon(kind: &str, tier: i32) -> Compound {
        let mut a = Compound::new();
        a.insert("kind".into(), Tag::String(kind.into()));
        a.insert("tier".into(), Tag::Int(tier));
        let mut data = Compound::new();
        data.insert(ADDON_KEY.into(), Tag::Compound(a));
        data
    }

    #[test]
    fn addons_keep_the_best_tier_of_each_kind_and_change_the_numbers() {
        let mut a = Addons::default();
        a.add_custom_data(&addon("speed", 1));
        a.add_custom_data(&addon("speed", 2));
        a.add_custom_data(&addon("efficiency", 3));
        a.add_custom_data(&addon("range", 4));
        a.add_custom_data(&addon("unknown", 9));
        a.add_custom_data(&Compound::new());
        assert_eq!(a, Addons { speed: 2, efficiency: 3, processing: 0, range: 4 });
        assert_eq!(a.progress_per_tick(), 3);
        assert_eq!(a.energy_per_operation(200), 140);
        assert_eq!(a.radius(1), 5);
        let max = Addons { efficiency: 30, ..Default::default() };
        assert_eq!(max.energy_per_operation(200), 20);
        assert_eq!(max.energy_per_operation(1), 1);
    }

    #[test]
    fn the_working_area_lies_behind_the_machine() {
        // Facing north (front towards -z): the area is south of it.
        let (min, max) = working_area([0, 64, 0], [0, 0, -1], 1, 1, 0);
        assert_eq!((min, max), ([-1, 64, 1], [1, 64, 3]));
        let (min, max) = working_area([10, 5, 10], [1, 0, 0], 2, 3, 0);
        assert_eq!((min, max), ([5, 5, 8], [9, 7, 12]));
        assert_eq!(area_size(min, max), 75);
    }

    #[test]
    fn the_area_scan_visits_every_position_once_then_wraps() {
        let (min, max) = ([0, 0, 0], [1, 1, 2]);
        let n = area_size(min, max);
        let mut seen: Vec<[i32; 3]> = (0..n).map(|i| area_position(min, max, i)).collect();
        assert_eq!(seen[0], [0, 0, 0]);
        assert_eq!(seen[1], [1, 0, 0]);
        assert_eq!(seen[2], [0, 0, 1]);
        assert_eq!(area_position(min, max, n), [0, 0, 0]);
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), n as usize);
    }

    #[test]
    fn tanks_take_one_fluid_up_to_capacity_and_round_trip() {
        let mut t = Tank::default();
        assert_eq!(t.fill("industrialforegoing:latex", 900, 1000), 900);
        assert_eq!(t.fill("industrialforegoing:latex", 900, 1000), 100);
        assert_eq!(t.fill("minecraft:water", 1, 1000), 0);
        assert_eq!(t.drain(1500), 1000);
        assert_eq!(t, Tank::default());
        let tanks = vec![Tank { fluid: Some("minecraft:water".into()), amount: 250 }, Tank::default()];
        let mut nbt = Compound::new();
        write_tanks(&mut nbt, &tanks);
        assert_eq!(read_tanks(&nbt, 3), vec![tanks[0].clone(), Tank::default(), Tank::default()]);
    }

    #[test]
    fn a_cycle_needs_work_and_energy_and_completes_at_its_max() {
        let mut p = 0;
        assert!(!cycle(&mut p, 10, 3, 100, 50, false));
        assert!(!cycle(&mut p, 10, 3, 10, 50, true));
        assert_eq!(p, 0);
        let done: Vec<bool> = (0..4).map(|_| cycle(&mut p, 10, 3, 100, 50, true)).collect();
        assert_eq!(done, [false, false, false, true]);
        assert_eq!(p, 0);
    }

    #[test]
    fn dissolution_needs_exactly_its_items_and_enough_fluid() {
        let items = |ids: &[&str]| -> Vec<Option<String>> {
            let mut v: Vec<Option<String>> = ids.iter().map(|s| Some(s.to_string())).collect();
            v.resize(8, None);
            v
        };
        let slime = Tank { fluid: Some("industrialforegoing:pink_slime".into()), amount: 1000 };
        let r = dissolution_match(&items(&["minecraft:gold_ingot", "minecraft:iron_ingot"]), &slime, DISSOLUTION_RECIPES);
        assert_eq!(r.and_then(|r| r.output).map(|o| o.0), Some("industrialforegoing:pink_slime_ingot"));
        assert!(dissolution_match(&items(&["minecraft:gold_ingot", "minecraft:iron_ingot", "minecraft:dirt"]), &slime, DISSOLUTION_RECIPES).is_none());
        let low = Tank { amount: 999, ..slime.clone() };
        assert!(dissolution_match(&items(&["minecraft:gold_ingot", "minecraft:iron_ingot"]), &low, DISSOLUTION_RECIPES).is_none());
    }

    #[test]
    fn a_black_hole_unit_takes_only_what_fits() {
        assert_eq!(black_hole_accepts(0, 64, 32_768), 64);
        assert_eq!(black_hole_accepts(32_760, 64, 32_768), 8);
        assert_eq!(black_hole_accepts(40_000, 64, 32_768), 0);
    }

    #[test]
    fn a_generator_burns_fuel_tick_by_tick_and_stops_when_full() {
        let mut burn = 0;
        assert_eq!(generator_tick(&mut burn, 0, &PITIFUL_GENERATOR, None), (0, false));
        assert_eq!(generator_tick(&mut burn, 0, &PITIFUL_GENERATOR, Some(2)), (30, true));
        assert_eq!(generator_tick(&mut burn, 30, &PITIFUL_GENERATOR, Some(2)), (30, false));
        assert_eq!(burn, 0);
        let mut burn = 5;
        assert_eq!(generator_tick(&mut burn, PITIFUL_GENERATOR.capacity - 10, &PITIFUL_GENERATOR, None), (0, false));
        assert_eq!(burn, 5);
    }
}
