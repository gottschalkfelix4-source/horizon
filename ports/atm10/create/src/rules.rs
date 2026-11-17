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
}

impl Kind {
    pub const ALL: [Kind; 12] = [
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
    if sails < MIN_SAILS { 0.0 } else { (sails as f64 / 2.0).min(16.0).floor() }
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
pub const MILL_WORK: f64 = 2048.0;
pub const PRESS_WORK: f64 = 1280.0;

/// The millstone's slots: 4 in, 5 out.
pub const MILL_SLOTS: usize = 9;
pub const MILL_INPUTS: usize = 4;

/// What a millstone makes of one item: `(output id, count)`.
pub fn mill(id: &str) -> Option<(&'static str, u32)> {
    Some(match id {
        "minecraft:wheat" => ("create:wheat_flour", 1),
        "minecraft:cobblestone" => ("minecraft:gravel", 1),
        "minecraft:bone" => ("minecraft:bone_meal", 3),
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
        _ => return None,
    })
}

/// Stress per RPM a mixer uses, its work, and the basin's nine slots.
pub const MIXER_IMPACT: f64 = 4.0;
pub const MIX_WORK: f64 = 2560.0;
pub const BASIN_SLOTS: usize = 9;

/// What a mixer makes of the items in a basin (unheated recipes only): the ids taken, one of
/// each, and the output `(id, count)`.
pub fn mix(held: &[String]) -> Option<(Vec<&'static str>, &'static str, u32)> {
    let has = |id: &str| held.iter().any(|h| h == id);
    if has("minecraft:iron_nugget") && has("minecraft:andesite") {
        return Some((vec!["minecraft:iron_nugget", "minecraft:andesite"], "create:andesite_alloy", 1));
    }
    None
}

/// One tick of work at `speed` RPM: the new progress and whether `work` is reached (then the
/// progress starts over). A block that stands still loses no progress.
pub fn advance(progress: f64, speed: f64, work: f64) -> (f64, bool) {
    let next = progress + speed.abs();
    if next >= work { (0.0, true) } else { (next, false) }
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
        assert_eq!(mill("minecraft:wheat"), Some(("create:wheat_flour", 1)));
        assert_eq!(mill("minecraft:dirt"), None);
        assert_eq!(press("minecraft:iron_ingot"), Some("create:iron_sheet"));
        assert_eq!(press("minecraft:iron_nugget"), None);
    }

    #[test]
    fn a_mixer_needs_both_ingredients() {
        let one = vec!["minecraft:andesite".to_string()];
        assert!(mix(&one).is_none());
        let both = vec!["minecraft:andesite".to_string(), "minecraft:iron_nugget".to_string()];
        assert_eq!(mix(&both).map(|m| (m.1, m.2)), Some(("create:andesite_alloy", 1)));
    }
}
