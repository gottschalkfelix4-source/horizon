//! Create's numbers for rotation, free of the game so they can be tested. Every figure is from
//! the default config as remembered and is listed in README.md under "To check".

/// The blocks of this slice.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Shaft,
    Cogwheel,
    LargeCogwheel,
    Gearbox,
    WaterWheel,
    WindmillBearing,
    CreativeMotor,
    Millstone,
    Depot,
    MechanicalPress,
    Basin,
    MechanicalMixer,
    Belt,
    Funnel,
    CrushingWheel,
    BlazeBurner,
    Deployer,
    MechanicalSaw,
}

impl Kind {
    pub const ALL: [Kind; 18] = [
        Kind::Shaft,
        Kind::Cogwheel,
        Kind::LargeCogwheel,
        Kind::Gearbox,
        Kind::WaterWheel,
        Kind::WindmillBearing,
        Kind::CreativeMotor,
        Kind::Millstone,
        Kind::Depot,
        Kind::MechanicalPress,
        Kind::Basin,
        Kind::MechanicalMixer,
        Kind::Belt,
        Kind::Funnel,
        Kind::CrushingWheel,
        Kind::BlazeBurner,
        Kind::Deployer,
        Kind::MechanicalSaw,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Shaft => "shaft",
            Kind::Cogwheel => "cogwheel",
            Kind::LargeCogwheel => "large_cogwheel",
            Kind::Gearbox => "gearbox",
            Kind::WaterWheel => "water_wheel",
            Kind::WindmillBearing => "windmill_bearing",
            Kind::CreativeMotor => "creative_motor",
            Kind::Millstone => "millstone",
            Kind::Depot => "depot",
            Kind::MechanicalPress => "mechanical_press",
            Kind::Basin => "basin",
            Kind::MechanicalMixer => "mechanical_mixer",
            Kind::Belt => "belt",
            Kind::Funnel => "funnel",
            Kind::CrushingWheel => "crushing_wheel",
            Kind::BlazeBurner => "blaze_burner",
            Kind::Deployer => "deployer",
            Kind::MechanicalSaw => "mechanical_saw",
        }
    }
}

/// The kind of a `create:` block id.
pub fn parse_block(id: &str) -> Option<Kind> {
    let name = id.strip_prefix("create:")?;
    Kind::ALL.into_iter().find(|k| k.name() == name)
}

/// Ticks between two solves of a network.
pub const SOLVE_EVERY: i32 = 10;

/// A water wheel's RPM with `flowing` sides in flowing water (none: stands still).
pub fn water_wheel_rpm(flowing: usize) -> f64 {
    if flowing == 0 { 0.0 } else { 8.0 }
}

/// Stress a water wheel supports per RPM.
pub const WATER_WHEEL_CAPACITY: f64 = 32.0;

/// The largest sail structure a windmill bearing counts.
pub const MAX_SAILS: usize = 64;

/// Fewest sail blocks a windmill needs to turn.
pub const MIN_SAILS: usize = 8;

/// A windmill's RPM: nothing below `MIN_SAILS`, then half a RPM per sail up to 16.
pub fn windmill_rpm(sails: usize) -> f64 {
    if sails < MIN_SAILS {
        0.0
    } else {
        (sails as f64 / 2.0).min(16.0).floor()
    }
}

/// Stress a windmill bearing supports per RPM.
pub const WINDMILL_CAPACITY: f64 = 64.0;

/// A creative motor's RPM and stress per RPM.
pub const CREATIVE_MOTOR_RPM: f64 = 16.0;
pub const CREATIVE_MOTOR_CAPACITY: f64 = 16384.0;

/// Whether `id` is a block a windmill counts as a sail: any wool, or Create's sail frame.
pub fn is_sail(id: &str) -> bool {
    id == "create:sail_frame" || id == "create:white_sail" || id.ends_with("_wool")
}

/// Stress per RPM a millstone and a press use.
pub const MILLSTONE_IMPACT: f64 = 4.0;
pub const PRESS_IMPACT: f64 = 8.0;

/// Work (RPM summed over ticks) one millstone item or one pressing needs: 64 ticks at 32 RPM.
pub const MILL_WORK: f64 = 2048.0; // the old flat time, kept for the tests of `advance`
pub const PRESS_WORK: f64 = 1280.0;

/// The millstone's slots: 4 in, 5 out.
pub const MILL_SLOTS: usize = 9;
pub const MILL_INPUTS: usize = 4;

/// Stress per RPM a crushing wheel uses.
pub const CRUSHER_IMPACT: f64 = 8.0;

/// Stress per RPM, and the work (RPM-ticks) of one deployer or saw cycle (both unconfirmed).
pub const DEPLOYER_IMPACT: f64 = 4.0;
pub const SAW_IMPACT: f64 = 4.0;
pub const DEPLOY_WORK: f64 = 640.0;

/// What a deployer makes of the item on a depot (`target`) with the item it holds (`held`): the
/// result id and whether the held item survives. From the original's deploying recipes: planks
/// turn a shaft into a cogwheel and a cogwheel into a large one; an axe scrapes and unwaxes copper
/// and keeps itself; honeycomb waxes copper and is used up.
pub fn deploy(target: &str, held: &str) -> Option<(String, bool)> {
    let is_planks = held.ends_with("_planks") && held.starts_with("minecraft:");
    match target {
        "create:shaft" if is_planks => return Some(("create:cogwheel".into(), false)),
        "create:cogwheel" if is_planks => return Some(("create:large_cogwheel".into(), false)),
        _ => {}
    }
    let name = target.strip_prefix("minecraft:")?;
    if !name.contains("copper") {
        return None;
    }
    if held.starts_with("minecraft:") && held.ends_with("_axe") {
        if let Some(bare) = name.strip_prefix("waxed_") {
            return Some((format!("minecraft:{bare}"), true));
        }
        for (stage, prev) in [("exposed_", ""), ("weathered_", "exposed_"), ("oxidized_", "weathered_")] {
            if let Some(rest) = name.strip_prefix(stage) {
                let out = if prev.is_empty() && rest == "copper" { "copper_block".to_string() } else { format!("{prev}{rest}") };
                return Some((format!("minecraft:{out}"), true));
            }
        }
        return None;
    }
    if held == "minecraft:honeycomb" && !name.starts_with("waxed_") {
        return Some((format!("minecraft:waxed_{name}"), false));
    }
    None
}

/// What a saw makes of one item: output id, count, ticks at 16 RPM.
pub fn cut(id: &str) -> Option<(&'static str, u32, u32)> {
    Some(match id {
        "create:andesite_alloy" => ("create:shaft", 6, 200),
        "minecraft:bamboo_planks" => ("minecraft:bamboo_mosaic", 1, 20),
        _ => return None,
    })
}

/// What a press makes of one item lying on a depot.
pub fn press(id: &str) -> Option<&'static str> {
    Some(match id {
        "minecraft:iron_ingot" => "create:iron_sheet",
        "minecraft:gold_ingot" => "create:golden_sheet",
        "minecraft:copper_ingot" => "create:copper_sheet",
        "create:brass_ingot" => "create:brass_sheet",
        "minecraft:sugar_cane" => "minecraft:paper",
        "create:pulp" => "create:cardboard",
        _ => return None,
    })
}

/// Stress per RPM a mixer uses, its work, and the basin's nine slots.
pub const MIXER_IMPACT: f64 = 4.0;
pub const MIX_WORK: f64 = 2560.0;
pub const BASIN_SLOTS: usize = 9;

/// Heat a blaze burner gives: 0 none, 1 heated (kindled), 2 superheated (seething).
pub const HEATED: u8 = 1;
pub const SUPERHEATED: u8 = 2;

/// What a mixer makes of the items in a basin at heat level `heat` (andesite plus an iron or zinc
/// nugget; brass from copper and zinc ingots needs "heated"; the recipes with water, milk or
/// lava need fluids): the ids taken, one of each, and the output `(id, count)`.
pub fn mix(held: &[String], heat: u8) -> Option<(Vec<&'static str>, &'static str, u32)> {
    let has = |id: &str| held.iter().any(|h| h == id);
    if has("minecraft:andesite") {
        for nugget in ["minecraft:iron_nugget", "create:zinc_nugget"] {
            if has(nugget) {
                return Some((vec![nugget, "minecraft:andesite"], "create:andesite_alloy", 1));
            }
        }
    }
    if heat >= HEATED && has("minecraft:copper_ingot") && has("create:zinc_ingot") {
        return Some((vec!["minecraft:copper_ingot", "create:zinc_ingot"], "create:brass_ingot", 2));
    }
    None
}

/// How many ticks a blaze burner burns one fuel item for, and the heat it gives (not confirmed).
pub fn burner_fuel(id: &str) -> Option<(i32, u8)> {
    match id {
        "minecraft:coal" | "minecraft:charcoal" => Some((1600, HEATED)),
        "create:blaze_cake" => Some((3000, SUPERHEATED)),
        _ => None,
    }
}

/// Work (RPM summed over ticks) for a belt to move its item one block, and the stress a belt uses.
pub const BELT_WORK: f64 = 256.0;
pub const BELT_IMPACT: f64 = 0.0;

/// The block offset a belt with the shaft axis `axis` moves items by at `speed`: along the other
/// horizontal axis, towards +, or towards - when the speed is negative.
pub fn belt_step(axis: crate::kinetics::Axis, speed: f64) -> Option<[i32; 3]> {
    use crate::kinetics::Axis;
    if speed == 0.0 {
        return None;
    }
    let sign = if speed > 0.0 { 1 } else { -1 };
    match axis {
        Axis::X => Some([0, 0, sign]),
        Axis::Z => Some([sign, 0, 0]),
        Axis::Y => None,
    }
}

/// Ticks between two moves of a funnel, and how many items it moves.
pub const FUNNEL_EVERY: i64 = 8;
pub const FUNNEL_ITEMS: u32 = 1;

/// One tick of work at `speed` RPM: the new progress and whether `work` is reached (then the
/// progress starts over). A block that stands still loses no progress.
pub fn advance(progress: f64, speed: f64, work: f64) -> (f64, bool) {
    let next = progress + speed.abs();
    if next >= work {
        (0.0, true)
    } else {
        (next, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_parse_back_to_their_kind() {
        for k in Kind::ALL {
            assert_eq!(parse_block(&format!("create:{}", k.name())), Some(k));
        }
        assert_eq!(parse_block("create:andesite_casing"), None);
        assert_eq!(parse_block("mekanism:shaft"), None);
    }

    #[test]
    fn a_windmill_needs_eight_sails() {
        assert_eq!(windmill_rpm(7), 0.0);
        assert_eq!(windmill_rpm(8), 4.0);
        assert_eq!(windmill_rpm(64), 16.0);
    }

    #[test]
    fn a_water_wheel_needs_flowing_water() {
        assert_eq!(water_wheel_rpm(0), 0.0);
        assert_eq!(water_wheel_rpm(2), 8.0);
    }

    #[test]
    fn wool_and_frames_are_sails() {
        assert!(is_sail("minecraft:red_wool"));
        assert!(is_sail("create:sail_frame"));
        assert!(!is_sail("minecraft:oak_planks"));
    }

    #[test]
    fn work_adds_up_by_speed_and_wraps_when_done() {
        let (p, done) = advance(0.0, 32.0, MILL_WORK);
        assert!(!done);
        assert_eq!(p, 32.0);
        assert_eq!(advance(2040.0, -16.0, MILL_WORK), (0.0, true));
        // 32 RPM finishes in 64 ticks
        let (mut p, mut ticks) = (0.0, 0);
        loop {
            ticks += 1;
            let (np, done) = advance(p, 32.0, MILL_WORK);
            p = np;
            if done {
                break;
            }
        }
        assert_eq!(ticks, 64);
    }

    #[test]
    fn recipes_know_their_inputs() {
        assert_eq!(press("minecraft:iron_ingot"), Some("create:iron_sheet"));
        assert_eq!(press("minecraft:iron_nugget"), None);
        assert_eq!(press("minecraft:sugar_cane"), Some("minecraft:paper"));
    }

    #[test]
    fn a_mixer_needs_both_ingredients() {
        let one = vec!["minecraft:andesite".to_string()];
        assert!(mix(&one, 0).is_none());
        let both = vec![
            "minecraft:andesite".to_string(),
            "minecraft:iron_nugget".to_string(),
        ];
        assert_eq!(
            mix(&both, 0).map(|m| (m.1, m.2)),
            Some(("create:andesite_alloy", 1))
        );
    }

    #[test]
    fn a_belt_moves_along_the_other_horizontal_axis() {
        use crate::kinetics::Axis;
        assert_eq!(belt_step(Axis::X, 16.0), Some([0, 0, 1]));
        assert_eq!(belt_step(Axis::Z, -16.0), Some([-1, 0, 0]));
        assert_eq!(belt_step(Axis::Y, 16.0), None);
        assert_eq!(belt_step(Axis::X, 0.0), None);
    }


    #[test]
    fn brass_needs_heat() {
        let held = vec!["minecraft:copper_ingot".to_string(), "create:zinc_ingot".to_string()];
        assert!(mix(&held, 0).is_none());
        assert_eq!(mix(&held, HEATED).map(|m| (m.1, m.2)), Some(("create:brass_ingot", 2)));
        assert_eq!(mix(&held, SUPERHEATED).map(|m| m.1), Some("create:brass_ingot"));
    }

    #[test]
    fn only_fuel_burns_in_a_burner() {
        assert_eq!(burner_fuel("minecraft:coal"), Some((1600, HEATED)));
        assert_eq!(burner_fuel("create:blaze_cake").map(|f| f.1), Some(SUPERHEATED));
        assert_eq!(burner_fuel("minecraft:dirt"), None);
    }

    #[test]
    fn deployer_recipes_follow_the_original() {
        assert_eq!(deploy("create:shaft", "minecraft:oak_planks"), Some(("create:cogwheel".into(), false)));
        assert_eq!(deploy("create:shaft", "minecraft:stone"), None);
        assert_eq!(deploy("minecraft:exposed_copper", "minecraft:iron_axe"), Some(("minecraft:copper_block".into(), true)));
        assert_eq!(deploy("minecraft:oxidized_cut_copper", "minecraft:stone_axe"), Some(("minecraft:weathered_cut_copper".into(), true)));
        assert_eq!(deploy("minecraft:waxed_exposed_copper", "minecraft:iron_axe"), Some(("minecraft:exposed_copper".into(), true)));
        assert_eq!(deploy("minecraft:cut_copper", "minecraft:honeycomb"), Some(("minecraft:waxed_cut_copper".into(), false)));
        assert_eq!(deploy("minecraft:copper_block", "minecraft:iron_axe"), None);
    }

    #[test]
    fn a_saw_cuts_alloy_into_six_shafts() {
        assert_eq!(cut("create:andesite_alloy"), Some(("create:shaft", 6, 200)));
        assert_eq!(cut("minecraft:dirt"), None);
    }
}
