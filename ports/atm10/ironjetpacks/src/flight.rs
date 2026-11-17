//! The jetpack table and the flight rules: pure logic, no game access.
//!
//! The numbers are placeholders shaped like Iron Jetpacks' built-in jetpack definitions (each has a
//! capacity, a use per tick, vertical speed and acceleration, sideways speed, hover speeds and
//! sprint multipliers). The real values come from the original's data and are listed in README.md
//! to check.

/// One jetpack definition.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jetpack {
    /// The id kept in the stack's `ironjetpacks:jetpack_id` component (`iron`).
    pub id: &'static str,
    pub tier: u8,
    /// Energy it holds, in FE; 0 for the creative jetpack, which never runs out.
    pub capacity: u32,
    /// FE used per tick at full throttle.
    pub usage: u32,
    pub speed_vertical: f64,
    pub accel_vertical: f64,
    pub speed_sideways: f64,
    pub sprint_speed_multi: f64,
    pub sprint_fuel_multi: f64,
    pub speed_hover_ascend: f64,
    pub speed_hover_descend: f64,
    pub speed_hover: f64,
}

impl Jetpack {
    pub fn creative(&self) -> bool {
        self.capacity == 0
    }
}

#[allow(clippy::too_many_arguments)]
const fn jetpack(
    id: &'static str,
    tier: u8,
    capacity: u32,
    usage: u32,
    speed_vertical: f64,
    accel_vertical: f64,
    speed_sideways: f64,
    speed_hover_ascend: f64,
    speed_hover_descend: f64,
    speed_hover: f64,
) -> Jetpack {
    Jetpack {
        id,
        tier,
        capacity,
        usage,
        speed_vertical,
        accel_vertical,
        speed_sideways,
        sprint_speed_multi: 1.3,
        sprint_fuel_multi: 2.0,
        speed_hover_ascend,
        speed_hover_descend,
        speed_hover,
    }
}

/// The built-in jetpacks (assumed values, README "To check").
pub const JETPACKS: &[Jetpack] = &[
    jetpack("wood", 0, 20_000, 32, 0.22, 0.10, 0.0118, 0.18, 0.14, 0.005),
    jetpack("stone", 1, 100_000, 70, 0.30, 0.12, 0.08, 0.20, 0.10, 0.005),
    jetpack(
        "copper", 1, 150_000, 80, 0.33, 0.13, 0.09, 0.22, 0.10, 0.005,
    ),
    jetpack("iron", 2, 400_000, 120, 0.41, 0.14, 0.14, 0.27, 0.22, 0.005),
    jetpack(
        "gold", 3, 2_000_000, 300, 0.48, 0.15, 0.15, 0.40, 0.30, 0.005,
    ),
    jetpack(
        "diamond", 4, 10_000_000, 700, 0.55, 0.16, 0.17, 0.45, 0.35, 0.005,
    ),
    jetpack(
        "emerald", 5, 50_000_000, 1_500, 0.80, 0.18, 0.19, 0.48, 0.38, 0.005,
    ),
    jetpack(
        "netherite",
        6,
        200_000_000,
        3_000,
        0.90,
        0.19,
        0.21,
        0.52,
        0.40,
        0.005,
    ),
    jetpack("creative", 7, 0, 0, 0.90, 0.19, 0.21, 0.52, 0.40, 0.005),
];

pub fn jetpack_by_id(id: &str) -> Option<&'static Jetpack> {
    JETPACKS.iter().find(|j| j.id == id)
}

/// What the player holds down this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    pub jump: bool,
    pub sneak: bool,
    pub sprint: bool,
    /// -1..1, forward positive.
    pub forward: f32,
    /// -1..1, left positive (vanilla's `xxa`).
    pub strafe: f32,
}

/// The switches a jetpack stack carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Switches {
    pub engine: bool,
    pub hover: bool,
    /// 0..=100 percent.
    pub throttle: u8,
}

impl Default for Switches {
    fn default() -> Self {
        Self {
            engine: true,
            hover: false,
            throttle: 100,
        }
    }
}

/// How much one throttle key press changes the throttle, in percent (assumed).
pub const THROTTLE_STEP: u8 = 5;

/// The player's motion this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thrust {
    /// The vertical velocity the jetpack sets.
    pub vy: f64,
    /// The push along the player's facing (forward, left), in blocks per tick.
    pub forward: f64,
    pub strafe: f64,
}

/// Whether the jetpack flies this tick: engine on, energy left (or creative), and jump held or
/// hovering in the air.
pub fn flying(
    jetpack: &Jetpack,
    switches: Switches,
    energy: u32,
    input: Input,
    on_ground: bool,
) -> bool {
    switches.engine
        && (jetpack.creative() || energy > 0)
        && (input.jump || (switches.hover && !on_ground))
}

/// The motion of one flying tick, from the player's vertical velocity `vy`. Jump climbs (slower in
/// hover mode); hover without jump sinks slowly, faster while sneaking. Climbing accelerates more
/// while the player is still falling or slow.
pub fn thrust(jetpack: &Jetpack, switches: Switches, input: Input, vy: f64) -> Thrust {
    let throttle = f64::from(switches.throttle.min(100)) / 100.0;
    let vy = if input.jump {
        let top = if switches.hover {
            jetpack.speed_hover_ascend
        } else {
            jetpack.speed_vertical
        } * throttle;
        let boost = if vy < 0.3 { 2.5 } else { 1.0 };
        (vy + jetpack.accel_vertical * throttle * boost).min(top)
    } else {
        let sink = if input.sneak {
            jetpack.speed_hover_descend
        } else {
            jetpack.speed_hover
        };
        (vy + jetpack.accel_vertical * throttle).min(-sink)
    };
    let sideways = jetpack.speed_sideways * throttle;
    let forward_speed = if input.sprint {
        sideways * jetpack.sprint_speed_multi
    } else {
        sideways
    };
    Thrust {
        vy,
        forward: f64::from(input.forward) * forward_speed,
        strafe: f64::from(input.strafe) * sideways,
    }
}

/// The world-space push of a (forward, left) move for a player facing `yaw` degrees, as
/// vanilla's `Entity.moveRelative` turns its input.
pub fn world_push(forward: f64, strafe: f64, yaw_degrees: f32) -> [f64; 2] {
    let yaw = f64::from(yaw_degrees).to_radians();
    let (sin, cos) = yaw.sin_cos();
    [strafe * cos - forward * sin, forward * cos + strafe * sin]
}

/// The FE one flying tick uses.
pub fn energy_use(jetpack: &Jetpack, switches: Switches, sprinting: bool) -> u32 {
    if jetpack.creative() {
        return 0;
    }
    let throttle = f64::from(switches.throttle.min(100)) / 100.0;
    let multi = if sprinting {
        jetpack.sprint_fuel_multi
    } else {
        1.0
    };
    (f64::from(jetpack.usage) * throttle * multi).round() as u32
}

/// The item bar for `energy` of `capacity`: width 0..=13 and the colour (assumed red).
pub fn energy_bar(energy: u32, capacity: u32) -> Option<(i32, u32)> {
    if capacity == 0 {
        return None;
    }
    let width = (13.0 * f64::from(energy.min(capacity)) / f64::from(capacity)).round() as i32;
    Some((width, 0xFF_00_00))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iron() -> &'static Jetpack {
        jetpack_by_id("iron").unwrap()
    }

    #[test]
    fn jump_climbs_with_a_boost_while_slow_up_to_the_top_speed() {
        let up = Input {
            jump: true,
            ..Input::default()
        };
        let jp = iron();
        let t = thrust(jp, Switches::default(), up, 0.0);
        assert!((t.vy - 0.35).abs() < 1e-9, "{}", t.vy);
        let t = thrust(jp, Switches::default(), up, 0.35);
        assert!((t.vy - 0.41).abs() < 1e-9, "{}", t.vy);
    }

    #[test]
    fn hover_sinks_slowly_and_faster_while_sneaking() {
        let jp = iron();
        let hover = Switches {
            hover: true,
            ..Switches::default()
        };
        assert_eq!(thrust(jp, hover, Input::default(), -1.0).vy, -0.86);
        assert_eq!(thrust(jp, hover, Input::default(), 0.0).vy, -0.005);
        let sneak = Input {
            sneak: true,
            ..Input::default()
        };
        assert_eq!(thrust(jp, hover, sneak, 0.0).vy, -0.22);
    }

    #[test]
    fn a_jetpack_flies_only_with_its_engine_on_and_energy_left() {
        let jp = iron();
        let jump = Input {
            jump: true,
            ..Input::default()
        };
        let on = Switches::default();
        let off = Switches {
            engine: false,
            ..on
        };
        assert!(flying(jp, on, 10, jump, true));
        assert!(!flying(jp, off, 10, jump, true));
        assert!(!flying(jp, on, 0, jump, true));
        assert!(flying(
            jetpack_by_id("creative").unwrap(),
            on,
            0,
            jump,
            true
        ));
        let hover = Switches { hover: true, ..on };
        assert!(flying(jp, hover, 10, Input::default(), false));
        assert!(!flying(jp, hover, 10, Input::default(), true));
    }

    #[test]
    fn throttle_and_sprint_scale_the_energy_used() {
        let jp = iron();
        let half = Switches {
            throttle: 50,
            ..Switches::default()
        };
        assert_eq!(energy_use(jp, Switches::default(), false), 120);
        assert_eq!(energy_use(jp, half, false), 60);
        assert_eq!(energy_use(jp, Switches::default(), true), 240);
        assert_eq!(
            energy_use(jetpack_by_id("creative").unwrap(), half, true),
            0
        );
    }

    #[test]
    fn a_forward_push_follows_the_facing() {
        // Yaw 0 faces +Z, yaw 90 faces -X (vanilla's convention).
        let [x, z] = world_push(1.0, 0.0, 0.0);
        assert!(x.abs() < 1e-9 && (z - 1.0).abs() < 1e-9);
        let [x, z] = world_push(1.0, 0.0, 90.0);
        assert!((x + 1.0).abs() < 1e-9 && z.abs() < 1e-9);
    }

    #[test]
    fn the_energy_bar_is_a_share_of_thirteen_pixels() {
        assert_eq!(energy_bar(200, 400).unwrap().0, 7);
        assert_eq!(energy_bar(0, 400).unwrap().0, 0);
        assert_eq!(energy_bar(0, 0), None);
    }
}
