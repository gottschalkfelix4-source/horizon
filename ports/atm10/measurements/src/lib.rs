//! Measurements for Horizon: a tape measure that gives the size of the box between two blocks.
//!
//! Written from Measurements' public description and observed behaviour, never from its code
//! (docs/modpacks.md, "Licences"). What a harness against the NeoForge original still has to
//! confirm is in README.md.
//!
//! How it works: right-clicking a block with the tape measure records it as the start on the
//! player (an attachment); right-clicking a second block tells the player the box's size along
//! each axis, its block count and the straight distance between the two blocks' centres, then
//! clears the start. Sneaking while clicking clears a start without measuring.
//!
//! The client follows the same clicks on its own (`use_on` runs on both sides) and draws the
//! box being measured, then the measured box until the next click, as world lines.
use std::sync::Mutex;

use mod_api::{
    AttachmentType, ClientHost, Holder, Host, HostSide, InteractionResult, ItemPart, PlayerRef,
    Registrar, RegistrationContext, RenderLevelStage, RenderLevelStageEvent, ServerHost, Text,
    UseOnEvent, WorldDepth, WorldLine,
};

/// The start the player picked, if any. Not saved: a start is forgotten on logout, as a
/// half-made measurement is of no use later.
static START: AttachmentType<Option<[i32; 3]>> = AttachmentType {
    namespace: "measurements",
    name: "start",
    default: || None,
    codec: None,
    copy_on_death: false,
    sync: false,
};

/// What a measurement between two blocks gives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measurement {
    /// Blocks along x, y and z, both ends counted.
    pub size: [u32; 3],
    /// How many blocks the box holds.
    pub blocks: u64,
    /// The straight distance between the two blocks' centres.
    pub distance: f64,
}

/// Measure the box whose opposite corners are the blocks `a` and `b`.
pub fn measure(a: [i32; 3], b: [i32; 3]) -> Measurement {
    let d = [0, 1, 2].map(|i| (i64::from(a[i]) - i64::from(b[i])).abs());
    let size = d.map(|v| (v + 1) as u32);
    Measurement {
        size,
        blocks: size.iter().map(|&s| u64::from(s)).product(),
        distance: d.iter().map(|&v| (v * v) as f64).sum::<f64>().sqrt(),
    }
}

/// The distance as the message shows it: two decimals, trailing zeros dropped.
pub fn format_distance(distance: f64) -> String {
    let text = format!("{distance:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// What a click with the tape measure does, given the start picked before.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Sneaking: forget the start.
    Clear,
    /// No start yet: this block is the start.
    Start([i32; 3]),
    /// A start was picked: measure from it to this block.
    Measure([i32; 3], [i32; 3]),
}

/// The step a click on `pos` takes.
pub fn step(start: Option<[i32; 3]>, pos: [i32; 3], sneaking: bool) -> Step {
    match (sneaking, start) {
        (true, _) => Step::Clear,
        (false, None) => Step::Start(pos),
        (false, Some(start)) => Step::Measure(start, pos),
    }
}

/// The twelve edges of the box that encloses the blocks `a` and `b`.
pub fn box_edges(a: [i32; 3], b: [i32; 3]) -> Vec<([f64; 3], [f64; 3])> {
    let lo = [0, 1, 2].map(|i| f64::from(a[i].min(b[i])));
    let hi = [0, 1, 2].map(|i| f64::from(a[i].max(b[i])) + 1.0);
    let corner = |m: usize| [0, 1, 2].map(|i| if m >> i & 1 == 1 { hi[i] } else { lo[i] });
    let mut edges = Vec::with_capacity(12);
    for m in 0..8usize {
        for axis in 0..3 {
            if m >> axis & 1 == 0 {
                edges.push((corner(m), corner(m | 1 << axis)));
            }
        }
    }
    edges
}

fn sneaking(host: &dyn Host, uuid: u128) -> bool {
    host.player(uuid)
        .and_then(|p| p.is_secondary_use_active(host))
        .unwrap_or(false)
}

/// The box a client shows: the start, and the end once measured.
type Shown = (Option<[i32; 3]>, Option<[i32; 3]>);

/// This client's box.
static SHOWN: Mutex<Shown> = Mutex::new((None, None));

fn follow_click(host: &dyn ClientHost, event: &UseOnEvent<'_>) {
    let Ok(mut shown) = SHOWN.lock() else {
        return;
    };
    let start = shown.0.filter(|_| shown.1.is_none());
    *shown = match step(start, event.pos, sneaking(host, event.player)) {
        Step::Clear => (None, None),
        Step::Start(pos) => (Some(pos), None),
        Step::Measure(a, b) => (Some(a), Some(b)),
    };
}

fn tell(host: &mut dyn ServerHost, player: PlayerRef, key: &str, args: Vec<String>) {
    let args = args.into_iter().map(Text::literal).collect();
    let _ = player.action_bar(host, Text::translate(key, args));
}

fn click(host: &mut dyn ServerHost, event: &UseOnEvent<'_>) {
    let Some(player) = host.player(event.player) else {
        return;
    };
    let holder = Holder::Player(event.player);
    let start = START.get(host.attachments(), holder).ok().flatten();
    match step(start, event.pos, sneaking(&*host, event.player)) {
        Step::Clear => {
            let _ = START.set(host.attachments(), holder, None);
            tell(host, player, "measurements.cleared", vec![]);
        }
        Step::Start(pos) => {
            let _ = START.set(host.attachments(), holder, Some(pos));
            let [x, y, z] = pos.map(|c| c.to_string());
            tell(host, player, "measurements.start", vec![x, y, z]);
        }
        Step::Measure(start, end) => {
            let _ = START.set(host.attachments(), holder, None);
            let m = measure(start, end);
            let [x, y, z] = m.size.map(|s| s.to_string());
            let args = vec![x, y, z, m.blocks.to_string(), format_distance(m.distance)];
            tell(host, player, "measurements.result", args);
        }
    }
}

struct TapeMeasure;

#[mod_api::horizon_part]
impl ItemPart for TapeMeasure {
    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        match host {
            HostSide::Server(host) => click(host, event),
            HostSide::Client(host) => follow_click(&*host, event),
        }
        Some(InteractionResult::Success)
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl mod_api::Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let item = r.item_key("measurements:tape_measure");
        r.item_part(item, TapeMeasure);
    }

    /// Draw the box being measured, through walls so it reads inside terrain.
    fn render_level_stage(&self, _: &dyn ClientHost, event: &mut RenderLevelStageEvent<'_>) {
        if event.stage != RenderLevelStage::AfterLevel {
            return;
        }
        let Some((a, b, color)) = SHOWN.lock().ok().and_then(|s| match *s {
            (Some(a), Some(b)) => Some((a, b, 0xFF_FF_D0_40)),
            (Some(a), None) => Some((a, a, 0xFF_40_C0_FF)),
            _ => None,
        }) else {
            return;
        };
        let lines: Vec<WorldLine> = box_edges(a, b)
            .into_iter()
            .map(|(from, to)| WorldLine::new(from, to, color, 2.0, WorldDepth::ThroughWalls))
            .collect();
        let _ = event.draw.lines(&lines);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_counts_both_end_blocks() {
        let m = measure([0, 64, 0], [2, 66, -4]);
        assert_eq!(m.size, [3, 3, 5]);
        assert_eq!(m.blocks, 45);
        // sqrt(2^2 + 2^2 + 4^2) = sqrt(24)
        assert!((m.distance - 4.898_979_485_566_356).abs() < 1e-12);
    }

    #[test]
    fn one_block_measures_one() {
        let m = measure([5, -10, 7], [5, -10, 7]);
        assert_eq!((m.size, m.blocks, m.distance), ([1, 1, 1], 1, 0.0));
    }

    #[test]
    fn clicks_start_measure_and_clear() {
        assert_eq!(step(None, [1, 2, 3], false), Step::Start([1, 2, 3]));
        assert_eq!(
            step(Some([0, 0, 0]), [1, 2, 3], false),
            Step::Measure([0, 0, 0], [1, 2, 3])
        );
        assert_eq!(step(Some([0, 0, 0]), [1, 2, 3], true), Step::Clear);
    }

    #[test]
    fn the_drawn_box_encloses_both_blocks() {
        let edges = box_edges([2, 5, -1], [0, 4, 1]);
        assert_eq!(edges.len(), 12);
        let mut total = 0.0;
        for (from, to) in &edges {
            for p in [from, to] {
                assert!([0.0, 3.0].contains(&p[0]));
                assert!([4.0, 6.0].contains(&p[1]));
                assert!([-1.0, 2.0].contains(&p[2]));
            }
            total += (0..3).map(|i| (to[i] - from[i]).abs()).sum::<f64>();
        }
        // Four edges of each length: 4 * (3 + 2 + 3).
        assert_eq!(total, 32.0);
    }

    #[test]
    fn distances_drop_trailing_zeros() {
        assert_eq!(format_distance(4.898_979), "4.9");
        assert_eq!(format_distance(12.0), "12");
        assert_eq!(format_distance(7.456), "7.46");
    }
}
