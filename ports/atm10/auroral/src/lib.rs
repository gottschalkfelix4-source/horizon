//! Auroral for Horizon: auroras in the night sky over cold biomes.
//!
//! Written from Auroral's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and what a harness against the NeoForge
//! original still has to confirm, is in README.md.
//!
//! How it works: at the sky stage of each world view (`render_level_stage` with `AfterSky`, so
//! terrain and clouds draw over it) the client works out how strong an aurora is where the
//! camera stands: only in the Overworld, only at night (fading in after dusk and out before
//! dawn), only in a cold biome, and weaker in rain. When it is above zero it draws curtains of
//! alpha-blended quads high in the northern sky, rippling with the game time.
use mod_api::{ClientHost, Hooks, RenderLevelStage, RenderLevelStageEvent, WorldDepth, WorldQuad};

/// Ticks in a day.
pub const DAY: i64 = 24_000;
/// The aurora fades in from here (just after sunset)...
pub const DUSK: i64 = 13_000;
/// ...and is gone by here (just before sunrise).
pub const DAWN: i64 = 23_000;
/// Ticks over which it fades in and out.
pub const FADE: i64 = 1_000;
/// A biome this cold or colder shows the full aurora (snowy biomes are 0.0 and below)...
pub const COLD: f32 = 0.0;
/// ...and one this warm or warmer shows none.
pub const MILD: f32 = 0.3;

/// The aurora's strength, 0 to 1, at `day_time` in a biome of `temperature` with `rain` (0 to 1).
pub fn strength(day_time: i64, temperature: f32, rain: f32) -> f32 {
    let t = day_time.rem_euclid(DAY);
    let night = if t <= DUSK || t >= DAWN {
        0.0
    } else {
        let edge = (t - DUSK).min(DAWN - t);
        (edge as f32 / FADE as f32).min(1.0)
    };
    let cold = ((MILD - temperature) / (MILD - COLD)).clamp(0.0, 1.0);
    let clear = 1.0 - rain.clamp(0.0, 1.0);
    night * cold * clear
}

/// Curtains drawn per view.
pub const CURTAINS: usize = 3;
/// Quads along each curtain.
pub const SEGMENTS: usize = 48;

/// Pack a colour with `alpha` (0 to 1) into `0xAARRGGBB`.
pub fn argb(rgb: u32, alpha: f32) -> u32 {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
    (a << 24) | (rgb & 0x00ff_ffff)
}

/// The aurora's quads around `camera` at `time` (ticks, fractional), at `strength`: each curtain
/// is a lower green band and an upper violet band, both rippling north and south as they run
/// east to west. None when the strength is 0.
pub fn curtains(camera: [f64; 3], time: f64, strength: f32) -> Vec<WorldQuad> {
    if strength <= 0.0 {
        return Vec::new();
    }
    let mut quads = Vec::with_capacity(CURTAINS * SEGMENTS * 2);
    for c in 0..CURTAINS {
        let c_f = c as f64;
        let distance = 180.0 + 45.0 * c_f;
        let base = camera[1] + 90.0 + 12.0 * c_f;
        let width = 520.0;
        let x0 = camera[0] - width / 2.0;
        let step = width / SEGMENTS as f64;
        let wave = |x: f64| {
            let phase = time / 90.0 + c_f * 1.7;
            18.0 * (x / 70.0 + phase).sin() + 7.0 * (x / 23.0 - phase * 1.3).sin()
        };
        let height = |x: f64| 34.0 + 10.0 * (x / 41.0 + time / 140.0 + c_f).sin();
        for s in 0..SEGMENTS {
            let (xa, xb) = (x0 + step * s as f64, x0 + step * (s + 1) as f64);
            let (za, zb) = (
                camera[2] - distance + wave(xa - camera[0]),
                camera[2] - distance + wave(xb - camera[0]),
            );
            let (ha, hb) = (height(xa - camera[0]), height(xb - camera[0]));
            // Thinner at both ends of the curtain.
            let edge = (s.min(SEGMENTS - 1 - s) as f32 / 6.0).min(1.0);
            let alpha = strength * edge;
            let mid = |h: f64| h * 0.45;
            quads.push(WorldQuad::new(
                [
                    [xa, base, za],
                    [xb, base, zb],
                    [xb, base + mid(hb), zb],
                    [xa, base + mid(ha), za],
                ],
                argb(0x3cf08c, alpha * 0.55),
                WorldDepth::Tested,
            ));
            quads.push(WorldQuad::new(
                [
                    [xa, base + mid(ha), za],
                    [xb, base + mid(hb), zb],
                    [xb, base + hb, zb],
                    [xa, base + ha, za],
                ],
                argb(0x8c5cf0, alpha * 0.3),
                WorldDepth::Tested,
            ));
        }
    }
    quads
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn render_level_stage(&self, host: &dyn ClientHost, event: &mut RenderLevelStageEvent<'_>) {
        if event.stage != RenderLevelStage::AfterSky {
            return;
        }
        let Some(level) = host.camera_entity().and_then(|e| e.level(host)) else {
            return;
        };
        if level.dimension().id(host).as_deref() != Some("minecraft:overworld") {
            return;
        }
        let Some(clock) = host.clock() else {
            return;
        };
        let camera = event.view.camera;
        let pos = camera.map(|v| v.floor() as i32);
        let temperature = level
            .client_biome(host, pos)
            .and_then(|biome| level.biome_properties(host, &biome))
            .map_or(MILD, |p| p.temperature);
        let rain = level.weather(host).map_or(0.0, |(rain, _)| rain);
        let s = strength(clock.day_time, temperature, rain);
        let time = event.view.game_time as f64 + f64::from(event.view.partial_tick);
        let quads = curtains(camera, time, s);
        if !quads.is_empty() {
            // A full view budget drops the aurora for this frame; nothing else depends on it.
            let _ = event.draw.quads(&quads);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_aurora_shows_on_cold_clear_nights_only() {
        // Midnight in a snowy biome, clear sky: full.
        assert_eq!(strength(18_000, -0.5, 0.0), 1.0);
        // Noon, or a warm biome: none.
        assert_eq!(strength(6_000, -0.5, 0.0), 0.0);
        assert_eq!(strength(18_000, 0.8, 0.0), 0.0);
        // Half rain halves it; half-way between cold and mild halves it too.
        assert_eq!(strength(18_000, -0.5, 0.5), 0.5);
        assert!((strength(18_000, 0.15, 0.0) - 0.5).abs() < 1e-6);
        // The day after: the same night.
        assert_eq!(strength(DAY + 18_000, -0.5, 0.0), 1.0);
    }

    #[test]
    fn it_fades_in_after_dusk_and_out_before_dawn() {
        assert_eq!(strength(DUSK, -1.0, 0.0), 0.0);
        assert!((strength(DUSK + 250, -1.0, 0.0) - 0.25).abs() < 1e-6);
        assert!((strength(DAWN - 500, -1.0, 0.0) - 0.5).abs() < 1e-6);
        assert_eq!(strength(DAWN, -1.0, 0.0), 0.0);
    }

    #[test]
    fn curtains_hang_high_in_the_north_and_follow_the_strength() {
        let camera = [100.0, 70.0, -40.0];
        assert!(curtains(camera, 0.0, 0.0).is_empty());
        let quads = curtains(camera, 1234.5, 1.0);
        assert_eq!(quads.len(), CURTAINS * SEGMENTS * 2);
        for q in &quads {
            for c in q.corners {
                assert!(c[2] < camera[2] - 100.0, "north of the camera");
                assert!(c[1] >= camera[1] + 90.0, "above the camera");
            }
        }
        // Half strength draws the same quads at half the alpha (the brightest is 0.55 * 255).
        let alpha = |qs: &[WorldQuad]| qs.iter().map(|q| q.color >> 24).max().unwrap();
        assert_eq!(alpha(&quads), 140);
        assert_eq!(alpha(&curtains(camera, 1234.5, 0.5)), 70);
    }
}
