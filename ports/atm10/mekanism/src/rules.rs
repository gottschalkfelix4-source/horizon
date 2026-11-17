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

/// What a block id is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Cube(Tier),
    Cable(Tier),
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
    Machine::ALL.into_iter().find(|m| m.name() == name).map(Kind::Machine)
}

/// The first processing machines: one input, one output, run by energy.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Machine {
    EnrichmentChamber,
    Crusher,
    EnergizedSmelter,
}

impl Machine {
    pub const ALL: [Machine; 3] =
        [Machine::EnrichmentChamber, Machine::Crusher, Machine::EnergizedSmelter];

    pub fn name(self) -> &'static str {
        match self {
            Machine::EnrichmentChamber => "enrichment_chamber",
            Machine::Crusher => "crusher",
            Machine::EnergizedSmelter => "energized_smelter",
        }
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
