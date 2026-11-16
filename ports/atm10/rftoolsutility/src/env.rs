//! The Environmental Controller's rules: which module gives which effect, what a tick of it
//! costs, and who is inside.

/// An environmental module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Module {
    /// Item path in the `rftoolsutility` namespace.
    pub item: &'static str,
    /// The effect it keeps on players in range (`minecraft:` id) and its amplifier.
    pub effect: &'static str,
    pub amplifier: i32,
    /// RF per tick at the default radius.
    pub rf_per_tick: i32,
}

pub const MODULES: [Module; 8] = [
    Module { item: "environmental_module_speed", effect: "minecraft:speed", amplifier: 1, rf_per_tick: 20 },
    Module { item: "environmental_module_haste", effect: "minecraft:haste", amplifier: 1, rf_per_tick: 20 },
    Module { item: "environmental_module_regeneration", effect: "minecraft:regeneration", amplifier: 0, rf_per_tick: 40 },
    Module { item: "environmental_module_night_vision", effect: "minecraft:night_vision", amplifier: 0, rf_per_tick: 10 },
    Module { item: "environmental_module_water_breathing", effect: "minecraft:water_breathing", amplifier: 0, rf_per_tick: 10 },
    Module { item: "environmental_module_slow_falling", effect: "minecraft:slow_falling", amplifier: 0, rf_per_tick: 10 },
    Module { item: "environmental_module_glowing", effect: "minecraft:glowing", amplifier: 0, rf_per_tick: 10 },
    Module { item: "environmental_module_saturation", effect: "minecraft:saturation", amplifier: 0, rf_per_tick: 40 },
];

pub const RADII: [i32; 5] = [10, 25, 50, 75, 100];
pub const DEFAULT_RADIUS: i32 = 50;
/// Ticks between two applications of the effects, and how long each application lasts.
pub const PERIOD: i32 = 10;
pub const EFFECT_TICKS: i32 = 40;

/// The module the item `path` (in `rftoolsutility`) is.
pub fn module_of(path: &str) -> Option<&'static Module> {
    MODULES.iter().find(|m| m.item == path)
}

/// RF per tick of the given modules at `radius`: the base cost, scaled linearly with the radius
/// against [`DEFAULT_RADIUS`].
pub fn cost_per_tick<'a>(modules: impl IntoIterator<Item = &'a Module>, radius: i32) -> i64 {
    let base: i64 = modules.into_iter().map(|m| i64::from(m.rf_per_tick)).sum();
    (base * i64::from(radius)).max(0) / i64::from(DEFAULT_RADIUS)
}

/// Whether `target` is inside the cube of `radius` blocks around `centre`.
pub fn in_range(centre: [i32; 3], target: [f64; 3], radius: i32) -> bool {
    let r = f64::from(radius);
    (0..3).all(|i| (target[i] - (f64::from(centre[i]) + 0.5)).abs() <= r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_adds_up_the_modules_and_grows_with_the_radius() {
        let two = [MODULES[0], MODULES[2]];
        assert_eq!(cost_per_tick(&two, 50), 60);
        assert_eq!(cost_per_tick(&two, 100), 120);
        assert_eq!(cost_per_tick(&two, 25), 30);
        assert_eq!(cost_per_tick(&[], 50), 0);
    }

    #[test]
    fn range_is_a_cube_around_the_block_centre() {
        assert!(in_range([0, 64, 0], [10.0, 64.0, -9.0], 10));
        assert!(!in_range([0, 64, 0], [10.6, 64.0, 0.0], 10));
        assert!(!in_range([0, 64, 0], [0.0, 80.0, 0.0], 10));
    }
}
