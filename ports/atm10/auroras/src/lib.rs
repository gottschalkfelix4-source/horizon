//! Auroras for Horizon: curtains of light over the clouds on clear nights in cold biomes, blue,
//! green and aqua, turning redder as the night goes on.
//!
//! Written from the mod's public description (its Modrinth and CurseForge pages), never from its
//! code (docs/modpacks.md, "Licences"). What still has to be checked against the original is in
//! README.md.
//!
//! The sky rules and the curtain geometry are plain functions, tested on their own; the hooks
//! read the client's time, weather and biome each tick and draw the curtains after the sky.
use std::sync::Mutex;

use mod_api::{
    ClientHost, ClientLevelUnloadEvent, ClientTickPostEvent, Hooks, RenderLevelStage,
    RenderLevelStageEvent, WorldDepth, WorldQuad,
};

/// Ticks in a Minecraft day.
const DAY: i64 = 24_000;
/// Sunset: the curtains start fading in (vanilla's day time of sunset is about 12,000).
const DUSK: i64 = 12_000;
/// Fully dark: the curtains are at their brightest from here...
const NIGHT: i64 = 13_000;
/// ...to here.
const LATE: i64 = 23_000;

/// How dark the sky is for an aurora, 0 by day to 1 between nightfall and the end of the night.
pub fn night_factor(day_time: i64) -> f32 {
    let t = day_time.rem_euclid(DAY);
    if t < DUSK {
        0.0
    } else if t < NIGHT {
        (t - DUSK) as f32 / (NIGHT - DUSK) as f32
    } else if t < LATE {
        1.0
    } else {
        (DAY - t) as f32 / (DAY - LATE) as f32
    }
}

/// How far the night has gone, 0 at sunset to 1 at sunrise: the curtains take a red tint.
pub fn night_progress(day_time: i64) -> f32 {
    let t = day_time.rem_euclid(DAY);
    if t < DUSK {
        0.0
    } else {
        (t - DUSK) as f32 / (DAY - DUSK) as f32
    }
}

/// A biome as cold as the snowy ones: precipitation falls as snow (`Biome.coldEnoughToSnow`).
const SNOWY: f32 = 0.15;
/// Cold biomes (taiga, windswept hills, stony shores) see auroras on some nights only.
const COLD: f32 = 0.3;
/// One night in this many has auroras in a cold, not snowy, biome.
const COLD_NIGHTS: u64 = 4;

/// Whether this biome temperature shows auroras on night number `night` (the day count).
/// Snowy biomes always do; cold ones on about one night in four, the same nights for everyone
/// in the world; warmer ones never.
pub fn climate_allows(temperature: f32, night: i64) -> bool {
    if temperature < SNOWY {
        return true;
    }
    if temperature >= COLD {
        return false;
    }
    // A fixed hash of the night number: SplitMix64's finaliser.
    let mut x = (night as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    x.is_multiple_of(COLD_NIGHTS)
}

/// How bright the auroras should be: dark sky, the climate, and no rain (rain and thunder
/// both hide them).
pub fn target_intensity(day_time: i64, temperature: f32, rain: f32) -> f32 {
    let night = day_time.div_euclid(DAY);
    // A night spans two day counts (it ends after midnight): count it from its sunset.
    let night = if day_time.rem_euclid(DAY) < DUSK {
        night - 1
    } else {
        night
    };
    if !climate_allows(temperature, night) {
        return 0.0;
    }
    night_factor(day_time) * (1.0 - rain.clamp(0.0, 1.0))
}

/// How fast the shown intensity follows the target, per tick (a fade of 2.5 seconds).
const FADE_PER_TICK: f32 = 0.02;

/// One tick's step of the shown intensity toward `target`.
pub fn approach(shown: f32, target: f32) -> f32 {
    if shown < target {
        (shown + FADE_PER_TICK).min(target)
    } else {
        (shown - FADE_PER_TICK).max(target)
    }
}

/// The curtains' bottom, just over vanilla's clouds (y 192).
const BOTTOM: f64 = 200.0;
/// Their height.
const HEIGHT: f64 = 60.0;
/// Distance between curtains along z: they run east to west.
const SPACING: i32 = 96;
/// How far around the camera curtains are drawn.
const RANGE: i32 = 192;
/// Length of one curtain segment along x.
const SEGMENT: i32 = 8;
/// Horizontal strips a curtain is cut into, brightest at the bottom.
const STRIPS: usize = 4;

/// The base colours, as RGB: blue, green, aqua.
const PALETTE: [[f32; 3]; 3] = [[0.25, 0.45, 1.0], [0.2, 1.0, 0.45], [0.2, 0.95, 0.9]];
/// The red the curtains' tops turn toward as the night goes on.
const RED: [f32; 3] = [1.0, 0.25, 0.35];

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

/// The colour of a curtain at world `x`, its `strip` (0 at the bottom), for a night `progress`
/// and an `intensity`, as 0xAARRGGBB.
pub fn color(x: f64, strip: usize, progress: f32, intensity: f32) -> u32 {
    // Blend through the palette every 128 blocks.
    let phase = (x / 128.0).rem_euclid(PALETTE.len() as f64) as f32;
    let i = phase.floor() as usize % PALETTE.len();
    let base = lerp3(PALETTE[i], PALETTE[(i + 1) % PALETTE.len()], phase.fract());
    let height = strip as f32 / (STRIPS - 1) as f32;
    let rgb = lerp3(base, RED, progress * height * 0.8);
    let alpha = intensity.clamp(0.0, 1.0) * 0.55 * (1.0 - height * 0.75);
    let [r, g, b] = rgb.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u32);
    let a = (alpha * 255.0).round() as u32;
    a << 24 | r << 16 | g << 8 | b
}

/// A curtain's sideways sway at world `x`, in blocks, at time `t` in ticks.
fn sway(x: f64, band: i32, t: f64) -> f64 {
    let b = f64::from(band);
    12.0 * (x / 70.0 + t / 400.0 + b * 1.7).sin() + 5.0 * (x / 23.0 - t / 150.0 + b * 0.6).sin()
}

/// The quads of every curtain near `camera` at time `t` (game ticks plus the partial tick).
/// Curtains are fixed in the world: walking moves past them, as past the clouds.
pub fn curtains(camera: [f64; 3], t: f64, progress: f32, intensity: f32) -> Vec<WorldQuad> {
    if intensity <= 0.0 {
        return Vec::new();
    }
    let cx = camera[0].floor() as i32;
    let cz = camera[2].floor() as i32;
    let first_band = (cz - RANGE + SPACING - 1).div_euclid(SPACING);
    let last_band = (cz + RANGE).div_euclid(SPACING);
    let start = (cx - RANGE).div_euclid(SEGMENT) * SEGMENT;
    let strip_height = HEIGHT / STRIPS as f64;
    let mut out = Vec::new();
    for band in first_band..=last_band {
        let z0 = f64::from(band * SPACING);
        let mut x = start;
        while x < cx + RANGE {
            let (xa, xb) = (f64::from(x), f64::from(x + SEGMENT));
            let (za, zb) = (z0 + sway(xa, band, t), z0 + sway(xb, band, t));
            for strip in 0..STRIPS {
                let y0 = BOTTOM + strip as f64 * strip_height;
                let y1 = y0 + strip_height;
                out.push(WorldQuad::new(
                    [[xa, y1, za], [xa, y0, za], [xb, y0, zb], [xb, y1, zb]],
                    color(xa, strip, progress, intensity),
                    WorldDepth::Tested,
                ));
            }
            x += SEGMENT;
        }
    }
    out
}

#[derive(Default)]
struct State {
    /// The intensity drawn, faded toward the target each tick.
    shown: f32,
    progress: f32,
}

#[derive(Default)]
struct Mod(Mutex<State>);

impl Mod {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(|poison| poison.into_inner())
    }
}

/// The target intensity where the camera is, or 0 where auroras can't be: no world, a
/// dimension with no sky or with a ceiling.
fn read_target(host: &dyn ClientHost) -> (f32, f32) {
    let Some(level) = host.level(host.dimension()) else {
        return (0.0, 0.0);
    };
    let open_sky = level
        .dimension_type(host)
        .is_some_and(|dim| dim.has_skylight && !dim.has_ceiling);
    let Some(day_time) = level.day_time(host) else {
        return (0.0, 0.0);
    };
    let at = host
        .camera()
        .map(|camera| camera.position)
        .map(|pos| pos.map(|axis| axis.floor() as i32));
    let temperature = at
        .and_then(|pos| level.client_biome(host, pos))
        .and_then(|biome| level.biome_properties(host, &biome))
        .map(|biome| biome.temperature);
    let (Some(temperature), true) = (temperature, open_sky) else {
        return (0.0, night_progress(day_time));
    };
    let rain = level.weather(host).map_or(0.0, |(rain, _)| rain);
    (
        target_intensity(day_time, temperature, rain),
        night_progress(day_time),
    )
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        let (target, progress) = read_target(&*host);
        let mut state = self.state();
        state.shown = approach(state.shown, target);
        state.progress = progress;
    }

    fn client_level_unload(&self, _: &mut dyn ClientHost, _: &mut ClientLevelUnloadEvent<'_>) {
        *self.state() = State::default();
    }

    fn render_level_stage(&self, _: &dyn ClientHost, event: &mut RenderLevelStageEvent<'_>) {
        if event.stage != RenderLevelStage::AfterSky {
            return;
        }
        let (shown, progress) = {
            let state = self.state();
            (state.shown, state.progress)
        };
        let t = event.view.game_time as f64 + f64::from(event.view.partial_tick);
        let mut quads = curtains(event.view.camera, t, progress, shown);
        if quads.is_empty() {
            return;
        }
        let usage = event.draw.usage();
        quads.truncate(usage.quad_capacity.saturating_sub(usage.quads) as usize);
        let _ = event.draw.quads(&quads);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auroras_shine_only_between_sunset_and_sunrise() {
        assert_eq!(night_factor(6_000), 0.0); // noon
        assert_eq!(night_factor(18_000), 1.0); // midnight
        assert_eq!(night_factor(12_500), 0.5); // half way through dusk
        assert_eq!(night_factor(24_000 + 18_000), 1.0); // the next midnight
    }

    #[test]
    fn snowy_biomes_always_cold_ones_sometimes_warm_ones_never() {
        // Snowy plains 0.0, taiga 0.25, plains 0.8.
        assert!((0..20).all(|night| climate_allows(0.0, night)));
        let taiga = (0..400)
            .filter(|&night| climate_allows(0.25, night))
            .count();
        assert!((50..150).contains(&taiga), "{taiga} of 400 nights");
        assert!((0..400).all(|night| !climate_allows(0.8, night)));
    }

    #[test]
    fn rain_hides_them_and_the_night_keeps_its_answer_past_midnight() {
        assert_eq!(target_intensity(18_000, 0.0, 1.0), 0.0);
        assert_eq!(target_intensity(18_000, 0.0, 0.0), 1.0);
        // A taiga night is the same night before and after midnight (day 0 sunset to day 1).
        let before = target_intensity(20_000, 0.25, 0.0) > 0.0;
        let after = target_intensity(24_000 + 500, 0.25, 0.0) > 0.0;
        assert_eq!(before, after);
    }

    #[test]
    fn the_top_reddens_as_the_night_goes_on() {
        let red = |argb: u32| (argb >> 16) & 0xFF;
        let early = color(0.0, STRIPS - 1, 0.0, 1.0);
        let late = color(0.0, STRIPS - 1, 1.0, 1.0);
        assert!(red(late) > red(early) + 80);
        // The bottom keeps its colour.
        assert_eq!(
            color(0.0, 0, 0.0, 1.0) & 0x00FF_FFFF,
            color(0.0, 0, 1.0, 1.0) & 0x00FF_FFFF
        );
    }

    #[test]
    fn curtains_hang_over_the_clouds_around_the_camera_and_fade_with_intensity() {
        assert!(curtains([0.0, 64.0, 0.0], 0.0, 0.0, 0.0).is_empty());
        let quads = curtains([1000.0, 64.0, -1000.0], 0.0, 0.0, 1.0);
        assert!(!quads.is_empty());
        for quad in &quads {
            for corner in quad.corners {
                assert!(corner[1] >= 192.0);
                assert!((corner[0] - 1000.0).abs() <= 208.0);
                assert!((corner[2] + 1000.0).abs() <= 230.0);
            }
        }
        let dim = curtains([1000.0, 64.0, -1000.0], 0.0, 0.0, 0.5);
        assert!(dim[0].color >> 24 < quads[0].color >> 24);
    }

    #[test]
    fn the_fade_moves_a_little_each_tick() {
        assert_eq!(approach(0.0, 1.0), FADE_PER_TICK);
        assert_eq!(approach(0.01, 0.0), 0.0);
    }
}
