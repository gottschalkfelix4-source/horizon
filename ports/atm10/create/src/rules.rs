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
}

impl Kind {
    pub const ALL: [Kind; 7] = [
        Kind::Shaft,
        Kind::Cogwheel,
        Kind::LargeCogwheel,
        Kind::Gearbox,
        Kind::WaterWheel,
        Kind::WindmillBearing,
        Kind::CreativeMotor,
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
}
