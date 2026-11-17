//! Create Additions' rules with no game in them: the block kinds and their numbers, the Tesla coil's
//! charging step and the motor and alternator conversion. The recipes' numbers (energy, charge rate)
//! come from the original's data files; the machines' own numbers are from memory of the default
//! config, and README.md lists them to check against the original.
use mod_api::serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Motor,
    Alternator,
    Coil,
    Accumulator,
    Connector,
    LargeConnector,
    Creative,
}

pub const ALL: [(&str, Kind); 8] = [
    ("electric_motor", Kind::Motor),
    ("alternator", Kind::Alternator),
    ("tesla_coil", Kind::Coil),
    ("modular_accumulator", Kind::Accumulator),
    ("connector", Kind::Connector),
    ("small_light_connector", Kind::Connector),
    ("large_connector", Kind::LargeConnector),
    ("creative_energy", Kind::Creative),
];

pub fn parse_block(id: &str) -> Option<Kind> {
    let name = id.strip_prefix("createaddition:")?;
    ALL.iter().find(|(n, _)| *n == name).map(|(_, k)| *k)
}

/// The Tesla coil's slots (a 3x3 menu): eight inputs and the output.
pub const COIL_SLOTS: usize = 9;
pub const COIL_OUTPUT: usize = 8;

pub const ACCUMULATOR_CAPACITY: i32 = 2_000_000;
pub const ACCUMULATOR_TRANSFER: i32 = 5_000;
pub const COIL_CAPACITY: i32 = 10_000;
pub const COIL_TRANSFER: i32 = 5_000;
pub const CONNECTOR_CAPACITY: i32 = 4_000;
pub const CONNECTOR_TRANSFER: i32 = 1_000;
pub const LARGE_CONNECTOR_CAPACITY: i32 = 16_000;
pub const LARGE_CONNECTOR_TRANSFER: i32 = 8_000;
pub const CREATIVE_OUTPUT: i32 = 1_000_000;
pub const MOTOR_CAPACITY: i32 = 8_000;
pub const ALTERNATOR_CAPACITY: i32 = 8_000;

/// The motor's speeds, an empty-handed use steps to the next.
pub const MOTOR_SPEEDS: [i32; 6] = [8, 16, 32, 64, 128, 256];
pub const MOTOR_DEFAULT_RPM: i32 = 32;
/// FE a motor draws per tick per RPM it turns; an alternator makes the same for each RPM it is turned,
/// less its loss.
pub const FE_PER_RPM: i32 = 8;
pub const ALTERNATOR_EFFICIENCY_PERCENT: i32 = 90;

pub fn next_motor_speed(rpm: i32) -> i32 {
    let at = MOTOR_SPEEDS
        .iter()
        .position(|s| *s == rpm)
        .map_or(0, |i| i + 1);
    MOTOR_SPEEDS[at % MOTOR_SPEEDS.len()]
}

/// What a motor turning at `rpm` draws in a tick.
pub fn motor_draw(rpm: i32) -> i32 {
    rpm.max(0) * FE_PER_RPM
}

/// What an alternator turned at `rpm` makes in a tick.
pub fn alternator_output(rpm: i32) -> i32 {
    rpm.max(0) * FE_PER_RPM * ALTERNATOR_EFFICIENCY_PERCENT / 100
}

/// A `createaddition:charging` recipe: what the Tesla coil charges into what.
#[derive(Clone, Debug, PartialEq)]
pub struct Charging {
    /// The ingredient as a recipe file writes it (an id, a `#tag` or a list), for the game's matcher.
    pub ingredient: String,
    /// The result as a recipe file writes it (`{"id": ..., "count": ...}`).
    pub result: String,
    /// FE the whole charge takes.
    pub energy: i64,
    /// Most FE per tick the item takes.
    pub max_rate: i32,
}

pub fn parse_charging(json: &Value) -> Option<Charging> {
    let ingredient = json.get("ingredients")?.as_array()?.first()?.to_string();
    let result = json.get("results")?.as_array()?.first()?.to_string();
    let energy = json.get("energy")?.as_i64()?;
    let max_rate = i32::try_from(json.get("max_charge_rate")?.as_i64()?).ok()?;
    (energy > 0 && max_rate > 0).then_some(Charging {
        ingredient,
        result,
        energy,
        max_rate,
    })
}

/// One tick of charging: with `progress` FE put in so far of `need`, a rate limit and `stored` FE in
/// the coil, how much is drawn and whether the item is done.
pub fn charge_step(progress: i64, need: i64, max_rate: i32, stored: i32) -> (i32, bool) {
    let left = (need - progress).max(0);
    let drawn = i64::from(max_rate.min(stored).max(0)).min(left) as i32;
    (drawn, progress + i64::from(drawn) >= need)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::serde_json::json;

    #[test]
    fn a_charging_recipe_of_the_original_reads_its_energy_and_rate() {
        let file = include_str!("../data/createaddition/recipe/charging/channeling.json");
        let recipe = parse_charging(&mod_api::serde_json::from_str(file).unwrap()).unwrap();
        assert_eq!((recipe.energy, recipe.max_rate), (1_000_000, 360));
        assert_eq!(recipe.ingredient, "\"minecraft:book\"");
        assert!(recipe.result.contains("minecraft:enchanted_book"));
        assert_eq!(
            parse_charging(&json!({"energy": 0, "max_charge_rate": 5})),
            None
        );
    }

    #[test]
    fn charging_is_limited_by_rate_by_the_stored_energy_and_ends_exactly() {
        assert_eq!(charge_step(0, 1000, 360, 10_000), (360, false));
        assert_eq!(charge_step(0, 1000, 360, 100), (100, false));
        assert_eq!(charge_step(900, 1000, 360, 10_000), (100, true));
        assert_eq!(charge_step(0, 1000, 360, 0), (0, false));
    }

    #[test]
    fn a_motor_draws_and_an_alternator_makes_energy_by_speed() {
        assert_eq!(motor_draw(32), 256);
        assert_eq!(alternator_output(32), 230);
        assert_eq!(alternator_output(-5), 0);
        // an alternator never makes what the motor that turns it drew
        assert!(
            MOTOR_SPEEDS
                .iter()
                .all(|r| alternator_output(*r) < motor_draw(*r))
        );
    }

    #[test]
    fn the_motor_steps_through_its_speeds_and_wraps() {
        assert_eq!(next_motor_speed(32), 64);
        assert_eq!(next_motor_speed(256), 8);
        assert_eq!(next_motor_speed(5), 8);
        assert_eq!(
            parse_block("createaddition:large_connector"),
            Some(Kind::LargeConnector)
        );
        assert_eq!(parse_block("create:shaft"), None);
    }
}
