//! Mekanism's numbers and recipe tables, free of the game so they can be tested. Every figure is
//! from the default config as remembered and is listed in README.md under "To check".

/// One conversion rate: Mekanism counts joules (J), the pack's default unit is FE; 1 FE = 2.5 J.
pub const J_PER_FE: f64 = 2.5;

/// Joules to FE, rounded down.
pub fn fe(joules: i64) -> i32 {
    (joules as f64 / J_PER_FE) as i32
}

/// Tiers of energy cubes and universal cables.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tier {
    Basic,
    Advanced,
    Elite,
    Ultimate,
}

impl Tier {
    pub const ALL: [Tier; 4] = [Tier::Basic, Tier::Advanced, Tier::Elite, Tier::Ultimate];

    pub fn name(self) -> &'static str {
        match self {
            Tier::Basic => "basic",
            Tier::Advanced => "advanced",
            Tier::Elite => "elite",
            Tier::Ultimate => "ultimate",
        }
    }

    pub fn from_name(name: &str) -> Option<Tier> {
        Tier::ALL.into_iter().find(|t| t.name() == name)
    }

    /// An energy cube's capacity in FE (2, 8, 32, 128 MJ).
    pub fn cube_capacity(self) -> i32 {
        fe(match self {
            Tier::Basic => 2_000_000,
            Tier::Advanced => 8_000_000,
            Tier::Elite => 32_000_000,
            Tier::Ultimate => 128_000_000,
        })
    }

    /// What an energy cube moves per tick, in FE (each side).
    pub fn cube_rate(self) -> i32 {
        fe(match self {
            Tier::Basic => 800,
            Tier::Advanced => 3_200,
            Tier::Elite => 12_800,
            Tier::Ultimate => 51_200,
        }) * 20
    }

    /// A universal cable's most per tick, in FE.
    pub fn cable_rate(self) -> i32 {
        fe(match self {
            Tier::Basic => 400,
            Tier::Advanced => 1_600,
            Tier::Elite => 6_400,
            Tier::Ultimate => 25_600,
        }) * 20
    }
}

/// The block ids: `mekanism:<tier>_energy_cube`, `mekanism:<tier>_universal_cable`.
pub fn cube_id(t: Tier) -> String {
    format!("mekanism:{}_energy_cube", t.name())
}

pub fn cable_id(t: Tier) -> String {
    format!("mekanism:{}_universal_cable", t.name())
}

/// What a pipe carries.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    /// Mechanical pipes: fluids.
    Pipe,
    /// Pressurized tubes: chemicals (gases and the rest).
    Tube,
    /// Logistical transporters: items.
    Transporter,
}

impl Family {
    pub const ALL: [Family; 3] = [Family::Pipe, Family::Tube, Family::Transporter];

    pub fn suffix(self) -> &'static str {
        match self {
            Family::Pipe => "mechanical_pipe",
            Family::Tube => "pressurized_tube",
            Family::Transporter => "logistical_transporter",
        }
    }

    /// The block entity type of the family.
    pub fn entity_type(self) -> &'static str {
        self.suffix()
    }

    /// What one network moves per tick at `t`: millibuckets, or items for transporters.
    pub fn rate(self, t: Tier) -> u32 {
        let step = match t {
            Tier::Basic => 1,
            Tier::Advanced => 4,
            Tier::Elite => 16,
            Tier::Ultimate => 64,
        };
        match self {
            Family::Pipe | Family::Tube => 1000 * step,
            Family::Transporter => match t {
                Tier::Basic => 1,
                Tier::Advanced => 2,
                Tier::Elite => 4,
                Tier::Ultimate => 8,
            },
        }
    }
}

pub fn pipe_id(f: Family, t: Tier) -> String {
    format!("mekanism:{}_{}", t.name(), f.suffix())
}

/// What a block id is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Cube(Tier),
    Cable(Tier),
    Pipe(Family, Tier),
    Machine(Machine),
}

pub fn parse_block(id: &str) -> Option<Kind> {
    let name = id.strip_prefix("mekanism:")?;
    if let Some(t) = name.strip_suffix("_energy_cube") {
        return Tier::from_name(t).map(Kind::Cube);
    }
    if let Some(t) = name.strip_suffix("_universal_cable") {
        return Tier::from_name(t).map(Kind::Cable);
    }
    for f in Family::ALL {
        if let Some(t) = name.strip_suffix(f.suffix()).and_then(|t| t.strip_suffix('_')) {
            return Tier::from_name(t).map(|t| Kind::Pipe(f, t));
        }
    }
    Machine::ALL.into_iter().find(|m| m.name() == name).map(Kind::Machine)
}

/// The first processing machines: one input, one output, run by energy.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Machine {
    EnrichmentChamber,
    Crusher,
    EnergizedSmelter,
    PrecisionSawmill,
    Combiner,
    MetallurgicInfuser,
}

impl Machine {
    pub const ALL: [Machine; 6] = [
        Machine::EnrichmentChamber,
        Machine::Crusher,
        Machine::EnergizedSmelter,
        Machine::PrecisionSawmill,
        Machine::Combiner,
        Machine::MetallurgicInfuser,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Machine::EnrichmentChamber => "enrichment_chamber",
            Machine::Crusher => "crusher",
            Machine::EnergizedSmelter => "energized_smelter",
            Machine::PrecisionSawmill => "precision_sawmill",
            Machine::Combiner => "combiner",
            Machine::MetallurgicInfuser => "metallurgic_infuser",
        }
    }

    /// Whether it has a second input (the combiner's cobblestone, the infuser's infusing item).
    pub fn second_input(self) -> bool {
        matches!(self, Machine::Combiner | Machine::MetallurgicInfuser)
    }

    /// Slots of its block entity.
    pub fn slots(self) -> usize {
        if self.second_input() { SLOTS_TWO } else { SLOTS }
    }

    /// The vanilla menu that shows them.
    pub fn menu(self) -> &'static str {
        if self.second_input() { "minecraft:generic_3x3" } else { "minecraft:hopper" }
    }

    /// Ticks one operation takes (200 for all three).
    pub fn ticks(self) -> i32 {
        200
    }

    /// FE per tick while working (50 J).
    pub fn fe_per_tick(self) -> i32 {
        fe(50)
    }

    /// The energy buffer in FE: 40 ticks of work (to check against the original).
    pub fn buffer(self) -> i32 {
        self.fe_per_tick() * 40
    }
}

/// Slots of a machine block entity: input, output, energy item, two upgrades.
pub const INPUT: usize = 0;
pub const OUTPUT: usize = 1;
pub const SLOTS: usize = 5;
/// Machines with a second input: slot 5 (the 3x3 menu's).
pub const INPUT2: usize = 5;
pub const SLOTS_TWO: usize = 9;

/// The materials Mekanism adds ores for or processes: (name, has an ore, is a vanilla metal).
pub const METALS: [&str; 5] = ["osmium", "tin", "lead", "uranium", "fluorite"];
pub const VANILLA_METALS: [&str; 4] = ["iron", "gold", "copper", "netherite"];

/// Enriching: what the enrichment chamber makes of `item`, and how many.
pub fn enrich(item: &str) -> Option<(String, u32)> {
    let name = item.strip_prefix("mekanism:").or_else(|| item.strip_prefix("minecraft:"))?;
    // ores and raw ore: two dusts
    for m in METALS.iter().filter(|m| **m != "fluorite") {
        if name == format!("{m}_ore") || name == format!("deepslate_{m}_ore") {
            return Some((format!("mekanism:dust_{m}"), 2));
        }
        if name == format!("raw_{m}") {
            return Some((format!("mekanism:dust_{m}"), 2));
        }
    }
    for m in ["iron", "gold", "copper"] {
        if name == format!("{m}_ore") || name == format!("deepslate_{m}_ore") {
            return Some((format!("mekanism:dust_{m}"), 2));
        }
        if name == format!("raw_{m}") {
            return Some((format!("mekanism:dust_{m}"), 2));
        }
    }
    match name {
        // fluorite ore gives 6 gems 
        "fluorite_ore" | "deepslate_fluorite_ore" => Some(("mekanism:fluorite_gem".into(), 6)),
        // enriched materials
        "redstone" => Some(("mekanism:enriched_redstone".into(), 1)),
        "coal" | "charcoal" => Some(("mekanism:enriched_carbon".into(), 1)),
        "diamond" => Some(("mekanism:enriched_diamond".into(), 1)),
        "glowstone_dust" => Some(("mekanism:enriched_glowstone".into(), 1)),
        "dust_iron" => Some(("mekanism:enriched_iron".into(), 1)),
        _ => None,
    }
}

/// Crushing: what the crusher makes of `item`, and how many.
pub fn crush(item: &str) -> Option<(String, u32)> {
    let name = item.strip_prefix("mekanism:").or_else(|| item.strip_prefix("minecraft:"))?;
    for m in METALS.iter().filter(|m| **m != "fluorite").chain(VANILLA_METALS.iter()) {
        if name == format!("ingot_{m}") || name == format!("{m}_ingot") {
            return Some((format!("mekanism:dust_{m}"), 1));
        }
    }
    match name {
        "cobblestone" => Some(("minecraft:gravel".into(), 1)),
        "stone" => Some(("minecraft:cobblestone".into(), 1)),
        "gravel" => Some(("minecraft:sand".into(), 1)),
        "sandstone" => Some(("minecraft:sand".into(), 2)),
        "cobbled_deepslate" => Some(("minecraft:gravel".into(), 1)),
        "bone" => Some(("minecraft:bone_meal".into(), 5)),
        "bone_block" => Some(("minecraft:bone_meal".into(), 9)),
        "blaze_rod" => Some(("minecraft:blaze_powder".into(), 4)),
        "coal" => Some(("mekanism:dust_coal".into(), 1)),
        "charcoal" => Some(("mekanism:dust_charcoal".into(), 1)),
        "obsidian" => Some(("mekanism:dust_obsidian".into(), 1)),
        "diamond" => Some(("mekanism:dust_diamond".into(), 1)),
        "lapis_lazuli" => Some(("mekanism:dust_lapis_lazuli".into(), 1)),
        "quartz" => Some(("mekanism:dust_quartz".into(), 1)),
        "emerald" => Some(("mekanism:dust_emerald".into(), 1)),
        _ => None,
    }
}

/// What a machine with an item recipe makes of its inputs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Recipe {
    pub output: String,
    pub count: u32,
    /// How many of the first and the second input it takes.
    pub take: [u32; 2],
    /// The infuse type and amount it spends (the metallurgic infuser).
    pub infuse: Option<(&'static str, u32)>,
}

fn name_of(id: &str) -> &str {
    id.strip_prefix("mekanism:").or_else(|| id.strip_prefix("minecraft:")).unwrap_or(id)
}

/// The precision sawmill: a log into six planks of its wood.
pub fn saw(item: &str) -> Option<Recipe> {
    let wood = item.strip_prefix("minecraft:")?;
    let stem = wood.strip_suffix("_log").or_else(|| wood.strip_suffix("_wood")).or_else(|| wood.strip_suffix("_stem")).or_else(|| wood.strip_suffix("_hyphae"))?;
    let stem = stem.strip_prefix("stripped_").unwrap_or(stem);
    Some(Recipe { output: format!("minecraft:{stem}_planks"), count: 6, take: [1, 0], infuse: None })
}

/// The combiner: a dust and cobblestone into the metal's ore.
pub fn combine(a: &str, b: &str) -> Option<Recipe> {
    if b != "minecraft:cobblestone" {
        return None;
    }
    let dust = name_of(a).strip_prefix("dust_")?;
    let ore = match dust {
        "osmium" | "tin" | "lead" | "uranium" => format!("mekanism:{dust}_ore"),
        "iron" | "gold" | "copper" => format!("minecraft:{dust}_ore"),
        _ => return None,
    };
    Some(Recipe { output: ore, count: 1, take: [1, 1], infuse: None })
}

/// What an item adds to the infuser's tank: its type and amount per item.
pub fn infuse_of(item: &str) -> Option<(&'static str, u32)> {
    Some(match name_of(item) {
        "redstone" => ("redstone", 10),
        "diamond" => ("diamond", 10),
        "dust_refined_obsidian" => ("refined_obsidian", 10),
        "coal" | "charcoal" | "enriched_carbon" => ("carbon", 10),
        "dust_tin" => ("tin", 10),
        "dust_gold" => ("gold", 10),
        _ => return None,
    })
}

/// The metallurgic infuser: an item and the tank's infuse type into an alloy or a dust.
pub fn infuse(item: &str, kind: &str, amount: u32) -> Option<Recipe> {
    let (out, ty, cost) = match name_of(item) {
        "iron_ingot" => ("mekanism:alloy_infused", "redstone", 10),
        "alloy_infused" => ("mekanism:alloy_reinforced", "diamond", 20),
        "alloy_reinforced" => ("mekanism:alloy_atomic", "refined_obsidian", 40),
        "enriched_iron" => ("mekanism:dust_steel", "carbon", 10),
        "copper_ingot" => ("mekanism:dust_bronze", "tin", 10),
        _ => return None,
    };
    (kind == ty && amount >= cost)
        .then(|| Recipe { output: out.into(), count: 1, take: [1, 0], infuse: Some((ty, cost)) })
}

/// Progress of one operation. `energy` is FE held; it goes down only while an operation runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Work {
    pub progress: i32,
}

/// What one tick of a machine does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Step {
    pub spent: i32,
    pub finished: bool,
}

/// One tick: `can_run` (a recipe whose result fits), the energy held, the machine's cost and length.
pub fn step(work: &mut Work, can_run: bool, energy: i32, per_tick: i32, ticks: i32) -> Step {
    if !can_run {
        work.progress = 0;
        return Step { spent: 0, finished: false };
    }
    if energy < per_tick {
        return Step { spent: 0, finished: false };
    }
    work.progress += 1;
    if work.progress >= ticks {
        work.progress = 0;
        return Step { spent: per_tick, finished: true };
    }
    Step { spent: per_tick, finished: false }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_basic_cube_holds_800_thousand_fe() {
        // 2 MJ at 2.5 J per FE
        assert_eq!(Tier::Basic.cube_capacity(), 800_000);
        assert_eq!(Tier::Ultimate.cube_capacity(), 51_200_000);
    }

    #[test]
    fn block_ids_round_trip() {
        assert_eq!(parse_block("mekanism:elite_energy_cube"), Some(Kind::Cube(Tier::Elite)));
        assert_eq!(parse_block("mekanism:basic_universal_cable"), Some(Kind::Cable(Tier::Basic)));
        assert_eq!(parse_block("mekanism:crusher"), Some(Kind::Machine(Machine::Crusher)));
        assert_eq!(
            parse_block("mekanism:ultimate_pressurized_tube"),
            Some(Kind::Pipe(Family::Tube, Tier::Ultimate))
        );
        assert_eq!(pipe_id(Family::Transporter, Tier::Elite), "mekanism:elite_logistical_transporter");
        assert_eq!(Family::Pipe.rate(Tier::Advanced), 4000);
        assert_eq!(parse_block("mekanism:osmium_ore"), None);
        assert_eq!(parse_block("powah:crusher"), None);
    }

    #[test]
    fn recipes_follow_the_chain() {
        assert_eq!(enrich("mekanism:osmium_ore"), Some(("mekanism:dust_osmium".into(), 2)));
        assert_eq!(enrich("minecraft:iron_ore"), Some(("mekanism:dust_iron".into(), 2)));
        assert_eq!(crush("minecraft:iron_ingot"), Some(("mekanism:dust_iron".into(), 1)));
        assert_eq!(crush("minecraft:cobblestone"), Some(("minecraft:gravel".into(), 1)));
        assert_eq!(crush("minecraft:dirt"), None);
    }

    #[test]
    fn the_sawmill_the_combiner_and_the_infuser_follow_their_chains() {
        assert_eq!(saw("minecraft:oak_log").map(|r| (r.output, r.count)), Some(("minecraft:oak_planks".into(), 6)));
        assert_eq!(saw("minecraft:stripped_crimson_stem").unwrap().output, "minecraft:crimson_planks");
        assert_eq!(saw("minecraft:stone"), None);
        assert_eq!(combine("mekanism:dust_osmium", "minecraft:cobblestone").unwrap().output, "mekanism:osmium_ore");
        assert_eq!(combine("mekanism:dust_iron", "minecraft:cobblestone").unwrap().output, "minecraft:iron_ore");
        assert_eq!(combine("mekanism:dust_osmium", "minecraft:dirt"), None);
        assert_eq!(infuse_of("minecraft:redstone"), Some(("redstone", 10)));
        let r = infuse("minecraft:iron_ingot", "redstone", 10).unwrap();
        assert_eq!((r.output.as_str(), r.infuse), ("mekanism:alloy_infused", Some(("redstone", 10))));
        assert_eq!(infuse("minecraft:iron_ingot", "redstone", 9), None);
        assert_eq!(infuse("minecraft:iron_ingot", "diamond", 100), None);
    }

    #[test]
    fn a_machine_needs_energy_every_tick_and_resets_without_a_recipe() {
        let mut w = Work::default();
        let per = Machine::Crusher.fe_per_tick();
        assert_eq!(step(&mut w, true, per - 1, per, 3), Step { spent: 0, finished: false });
        assert_eq!(w.progress, 0);
        step(&mut w, true, 100, per, 3);
        step(&mut w, true, 100, per, 3);
        assert_eq!(w.progress, 2);
        let s = step(&mut w, true, 100, per, 3);
        assert!(s.finished && w.progress == 0);
        step(&mut w, true, 100, per, 3);
        step(&mut w, false, 100, per, 3);
        assert_eq!(w.progress, 0);
    }
}
