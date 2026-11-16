//! Colorful Hearts for Horizon: health above 20 is not stacked in rows of hearts but drawn in
//! one row, each further 20 points in another colour over the last; absorption the same.
//!
//! Written from Colorful Hearts' observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and what a comparison with the NeoForge
//! original still has to confirm, is in README.md.
//!
//! The `health_hud` hook replaces the whole health block (armour, hearts, food, air), so the
//! rows other than the hearts are drawn as vanilla's `Gui.renderPlayerHealth` draws them; the
//! first 20 points of health use vanilla's own heart sprites, so a player at 20 or less sees no
//! change. Coloured layers are drawn as fills inside vanilla's heart container.
use std::borrow::Cow;
use std::sync::Mutex;

use mod_api::{
    ClientHost, GuiDraw, HealthHudEvent, HealthInputs, Hooks, LegacyRandom, RandomSource,
    Registrar, RegistrationContext, SettingSpec, SoundPlay,
    client::hud_layout::{Blend, HeartKind},
};

pub const MOD_ID: &str = "colorfulhearts";

/// Health points one row of ten hearts holds.
pub const ROW: i32 = 20;

/// Colours of health layers 2, 3, ... (layer 1 is vanilla's red), cycling past the last.
pub const HEALTH_COLORS: [u32; 7] = [
    0xF0_6E_14, // orange
    0xF5_DC_23, // yellow
    0x2D_B9_28, // green
    0x1E_AE_DB, // cyan
    0x71_35_DB, // purple
    0xDB_1E_C4, // pink
    0xE6_E6_E6, // white
];

/// Colours of absorption layers 2, 3, ... (layer 1 is vanilla's gold).
pub const ABSORPTION_COLORS: [u32; 6] = [
    0xE1_FA_9B, 0xA0_FF_AF, 0xAA_FF_FA, 0xAA_CD_FF, 0xD7_B4_FF, 0xFA_A5_FF,
];

/// What a heart slot shows: nothing, or a full or half heart of a layer, over a full heart
/// of the layer below when it is half.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    /// The layer of the full heart drawn first (0 = vanilla's), if any.
    pub under: Option<u32>,
    /// The layer and whether it is a half heart drawn on top, if any.
    pub top: Option<(u32, bool)>,
}

/// Slot `c` (0 = leftmost) of a row showing `points` half hearts in layers of 20.
pub fn slot(points: i32, c: i32) -> Slot {
    if points <= 0 {
        return Slot {
            under: None,
            top: None,
        };
    }
    let layer = (points - 1) / ROW;
    let rem = points - layer * ROW;
    let below = (layer > 0).then(|| layer as u32 - 1);
    let half = c * 2 + 1;
    if half < rem {
        Slot {
            under: None,
            top: Some((layer as u32, false)),
        }
    } else if half == rem {
        Slot {
            under: below,
            top: Some((layer as u32, true)),
        }
    } else {
        Slot {
            under: below,
            top: None,
        }
    }
}

/// The colour of `layer` (≥ 1) from `colors`, cycling.
pub fn layer_color(colors: &[u32], layer: u32) -> u32 {
    colors[(layer as usize - 1) % colors.len()]
}

/// The heart's inside, within its 9x9 container: rows of `(y, x0, x1)` inclusive.
const HEART: [(i32, i32, i32); 8] = [
    (1, 1, 2),
    (1, 6, 7),
    (2, 1, 3),
    (2, 5, 7),
    (3, 1, 7),
    (4, 2, 6),
    (5, 3, 5),
    (6, 4, 4),
];

fn shade(rgb: u32, by: f32) -> u32 {
    let ch = |s: u32| {
        let v = ((rgb >> s) & 0xFF) as f32;
        let v = if by >= 0.0 {
            v + (255.0 - v) * by
        } else {
            v * (1.0 + by)
        };
        (v.round().clamp(0.0, 255.0) as u32) << s
    };
    0xFF00_0000 | ch(16) | ch(8) | ch(0)
}

/// A coloured heart's inside at `(x, y)`: the left half only when `half`.
fn coloured(out: &mut Vec<GuiDraw>, x: i32, y: i32, rgb: u32, half: bool) {
    let body = shade(rgb, 0.0);
    for (dy, x0, x1) in HEART {
        let x1 = if half { x1.min(4) } else { x1 };
        if x0 > x1 {
            continue;
        }
        let color = if dy >= 5 { shade(rgb, -0.25) } else { body };
        out.push(GuiDraw::Fill {
            x0: x + x0,
            y0: y + dy,
            x1: x + x1 + 1,
            y1: y + dy + 1,
            color,
        });
    }
    // The highlight vanilla's hearts carry on their upper left.
    out.push(GuiDraw::Fill {
        x0: x + 2,
        y0: y + 2,
        x1: x + 3,
        y1: y + 3,
        color: shade(rgb, 0.6),
    });
}

fn sprite(name: &'static str, x: i32, y: i32) -> GuiDraw {
    GuiDraw::Sprite {
        sprite: Cow::Borrowed(name),
        x,
        y,
        w: 9,
        h: 9,
        region: None,
        color: 0xFFFF_FFFF,
        blend: Blend::Textured,
    }
}

/// Vanilla's heart sprites (`Gui.HeartType`), by kind, absorbing, hardcore, half and blinking.
fn heart(
    kind: Option<HeartKind>,
    absorbing: bool,
    hardcore: bool,
    half: bool,
    blink: bool,
) -> &'static str {
    let table: [&'static str; 8] = match (kind, absorbing) {
        (None, _) => [
            "hud/heart/container",
            "hud/heart/container_blinking",
            "hud/heart/container",
            "hud/heart/container_blinking",
            "hud/heart/container_hardcore",
            "hud/heart/container_hardcore_blinking",
            "hud/heart/container_hardcore",
            "hud/heart/container_hardcore_blinking",
        ],
        (_, true) => [
            "hud/heart/absorbing_full",
            "hud/heart/absorbing_full_blinking",
            "hud/heart/absorbing_half",
            "hud/heart/absorbing_half_blinking",
            "hud/heart/absorbing_hardcore_full",
            "hud/heart/absorbing_hardcore_full_blinking",
            "hud/heart/absorbing_hardcore_half",
            "hud/heart/absorbing_hardcore_half_blinking",
        ],
        (Some(HeartKind::Normal), _) => [
            "hud/heart/full",
            "hud/heart/full_blinking",
            "hud/heart/half",
            "hud/heart/half_blinking",
            "hud/heart/hardcore_full",
            "hud/heart/hardcore_full_blinking",
            "hud/heart/hardcore_half",
            "hud/heart/hardcore_half_blinking",
        ],
        (Some(HeartKind::Poisoned), _) => [
            "hud/heart/poisoned_full",
            "hud/heart/poisoned_full_blinking",
            "hud/heart/poisoned_half",
            "hud/heart/poisoned_half_blinking",
            "hud/heart/poisoned_hardcore_full",
            "hud/heart/poisoned_hardcore_full_blinking",
            "hud/heart/poisoned_hardcore_half",
            "hud/heart/poisoned_hardcore_half_blinking",
        ],
        (Some(HeartKind::Withered), _) => [
            "hud/heart/withered_full",
            "hud/heart/withered_full_blinking",
            "hud/heart/withered_half",
            "hud/heart/withered_half_blinking",
            "hud/heart/withered_hardcore_full",
            "hud/heart/withered_hardcore_full_blinking",
            "hud/heart/withered_hardcore_half",
            "hud/heart/withered_hardcore_half_blinking",
        ],
        (Some(HeartKind::Frozen), _) => [
            "hud/heart/frozen_full",
            "hud/heart/frozen_full_blinking",
            "hud/heart/frozen_half",
            "hud/heart/frozen_half_blinking",
            "hud/heart/frozen_hardcore_full",
            "hud/heart/frozen_hardcore_full_blinking",
            "hud/heart/frozen_hardcore_half",
            "hud/heart/frozen_hardcore_half_blinking",
        ],
    };
    table[usize::from(hardcore) * 4 + usize::from(half) * 2 + usize::from(blink)]
}

/// Higher layers of an effect's hearts keep one tint of the effect, lighter per layer.
fn effect_color(kind: HeartKind, layer: u32) -> Option<u32> {
    let base = match kind {
        HeartKind::Normal => return None,
        HeartKind::Poisoned => 0x94_8A_1F,
        HeartKind::Withered => 0x3C_3C_3C,
        HeartKind::Frozen => 0x9A_D6_F0,
    };
    let lighter = shade(base, (layer as f32 * 0.12).min(0.6));
    Some(lighter & 0x00FF_FFFF)
}

/// Draws one slot's hearts of a row.
#[allow(clippy::too_many_arguments)]
fn draw_slot(
    out: &mut Vec<GuiDraw>,
    s: Slot,
    x: i32,
    y: i32,
    i: &HealthInputs,
    absorbing: bool,
    blink: bool,
) {
    let kind = if absorbing && i.kind != HeartKind::Withered {
        None
    } else {
        Some(i.kind)
    };
    let mut one = |layer: u32, half: bool| {
        if layer == 0 {
            let name = match kind {
                None => heart(Some(i.kind), true, i.hardcore, half, false),
                Some(k) => heart(Some(k), absorbing, i.hardcore, half, blink),
            };
            out.push(sprite(name, x, y));
        } else {
            let rgb = match (absorbing, effect_color(i.kind, layer)) {
                (false, Some(rgb)) => rgb,
                (false, None) => layer_color(&HEALTH_COLORS, layer),
                (true, _) => layer_color(&ABSORPTION_COLORS, layer),
            };
            coloured(out, x, y, rgb, half);
        }
    };
    if let Some(layer) = s.under {
        one(layer, false);
    }
    if let Some((layer, half)) = s.top {
        one(layer, half);
    }
}

/// State kept between frames (`Gui.lastHealth`, `displayHealth`...).
#[derive(Clone, Debug, Default)]
pub struct State {
    last_health: i32,
    display_health: i32,
    last_health_time: i64,
    health_blink_time: i64,
    last_bubble_pop: i32,
}

/// What one frame draws, and the bubble-pop sound (volume, pitch) it asks for.
pub struct Frame {
    pub draws: Vec<GuiDraw>,
    pub bubble_pop: Option<(f32, f32)>,
}

fn ceil(v: f32) -> i32 {
    v.ceil() as i32
}

impl State {
    /// The health block for a GUI of `w` x `h`: vanilla's rows, with hearts in one row
    /// (absorption in a second row above it, or over it when `absorption_over`).
    pub fn layout(&mut self, i: &HealthInputs, w: i32, h: i32, absorption_over: bool) -> Frame {
        let mut out = Vec::new();
        let mut bubble_pop = None;
        let tick = i64::from(i.tick);
        let current = ceil(i.health);
        let blink = self.health_blink_time > tick && (self.health_blink_time - tick) / 3 % 2 == 1;
        if current < self.last_health && i.damage_cooldown {
            self.last_health_time = i.millis;
            self.health_blink_time = tick + 20;
        } else if current > self.last_health && i.damage_cooldown {
            self.last_health_time = i.millis;
            self.health_blink_time = tick + 10;
        }
        if i.millis - self.last_health_time > 1000 {
            self.display_health = current;
            self.last_health_time = i.millis;
        }
        self.last_health = current;
        let old = self.display_health;
        let mut random = LegacyRandom::new(i64::from(i.tick.wrapping_mul(312_871)));
        let mut next = |bound: i32| random.next_int_bound(bound).unwrap_or(0);
        let (x_left, x_right, y_base) = (w / 2 - 91, w / 2 + 91, h - 39);
        let max_health = i.max_health.max(old.max(current) as f32);
        let absorption = ceil(i.absorption);
        let absorb_row = absorption > 0 && !absorption_over;
        let rows = 1 + i32::from(absorb_row);
        let row_height = 10;
        let mut y_air = y_base - 10;
        let containers = ceil(max_health / 2.0).min(10);
        let offset_index = if i.regeneration {
            i.tick % ceil(max_health.min(ROW as f32) + 5.0)
        } else {
            -1
        };
        if i.armor > 0 {
            let y = y_base - (rows - 1) * row_height - 10;
            for n in 0..10 {
                let x = x_left + n * 8;
                let name = if n * 2 + 1 < i.armor {
                    "hud/armor_full"
                } else if n * 2 + 1 == i.armor {
                    "hud/armor_half"
                } else {
                    "hud/armor_empty"
                };
                out.push(sprite(name, x, y));
            }
        }
        let low = current + absorption <= 4;
        // Health row.
        for c in (0..containers).rev() {
            let x = x_left + c * 8;
            let mut y = y_base;
            if low {
                y += next(2);
            }
            if c == offset_index {
                y -= 2;
            }
            out.push(sprite(heart(None, false, i.hardcore, false, blink), x, y));
            if blink && old <= ROW && c * 2 < old {
                let name = heart(Some(i.kind), false, i.hardcore, c * 2 + 1 == old, true);
                out.push(sprite(name, x, y));
            }
            draw_slot(&mut out, slot(current, c), x, y, i, false, false);
            if absorption_over {
                draw_slot(&mut out, slot(absorption, c), x, y, i, true, false);
            }
        }
        // Absorption row above it.
        if absorb_row {
            let absorb_containers = ceil(absorption as f32 / 2.0).min(10);
            for c in (0..absorb_containers).rev() {
                let x = x_left + c * 8;
                let mut y = y_base - row_height;
                if low {
                    y += next(2);
                }
                out.push(sprite(heart(None, false, i.hardcore, false, false), x, y));
                draw_slot(&mut out, slot(absorption, c), x, y, i, true, false);
            }
        }
        // Food, as vanilla.
        if i.vehicle_hearts == 0 {
            let (empty, half, full) = if i.hunger {
                (
                    "hud/food_empty_hunger",
                    "hud/food_half_hunger",
                    "hud/food_full_hunger",
                )
            } else {
                ("hud/food_empty", "hud/food_half", "hud/food_full")
            };
            for n in 0..10 {
                let mut y = y_base;
                if i.saturation <= 0.0 && i.tick % (i.food * 3 + 1) == 0 {
                    y += next(3) - 1;
                }
                let x = x_right - n * 8 - 9;
                out.push(sprite(empty, x, y));
                if n * 2 + 1 < i.food {
                    out.push(sprite(full, x, y));
                }
                if n * 2 + 1 == i.food {
                    out.push(sprite(half, x, y));
                }
            }
            y_air -= 10;
        }
        // Air, as vanilla.
        let max_air = i.max_air.max(1);
        let air = i.air.clamp(0, max_air);
        if i.under_water || air < max_air {
            let vehicle_rows = (f64::from(i.vehicle_hearts) / 10.0).ceil() as i32;
            let y = y_air - (vehicle_rows - 1) * 10;
            let bubble = |offset: i32| ceil(((air + offset) * 10) as f32 / max_air as f32);
            let full = bubble(-2);
            let popping = bubble(0);
            let delay = i32::from(air != 0 && i.under_water);
            let empty = 10 - bubble(delay);
            if !i.under_water {
                self.last_bubble_pop = 0;
            }
            for b in 1..=10 {
                let x = x_right - (b - 1) * 8 - 9;
                if b <= full {
                    out.push(sprite("hud/air", x, y));
                } else if full != popping && b == popping && i.under_water {
                    out.push(sprite("hud/air_bursting", x, y));
                    if self.last_bubble_pop != b {
                        let volume = 0.5 + 0.1 * (empty - 3 + 1).max(0) as f32;
                        let pitch = 1.0 + 0.1 * (empty - 5 + 1).max(0) as f32;
                        bubble_pop = Some((volume, pitch));
                        self.last_bubble_pop = b;
                    }
                } else if b > 10 - empty {
                    let wobble = if empty == 10 && i.tick % 2 == 0 {
                        next(2)
                    } else {
                        0
                    };
                    out.push(sprite("hud/air_empty", x, y + wobble));
                }
            }
        }
        Frame {
            draws: out,
            bubble_pop,
        }
    }
}

#[derive(Default)]
struct Mod {
    state: Mutex<State>,
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let label = "colorfulhearts.config.absorption_over_health";
        let _ = r.register_setting(
            SettingSpec::toggle("absorption_over_health", label, false)
                .tooltip(format!("{label}.tooltip")),
        );
    }

    fn health_hud(
        &self,
        host: &mut dyn ClientHost,
        event: &mut HealthHudEvent<'_>,
    ) -> Option<Vec<GuiDraw>> {
        if !host.on(MOD_ID) {
            return None;
        }
        let over = host.setting_bool("absorption_over_health").unwrap_or(false);
        let frame = self
            .state
            .lock()
            .ok()?
            .layout(event.inputs, event.wi, event.hi, over);
        if let Some((volume, pitch)) = frame.bubble_pop {
            let mut sound = SoundPlay::ui("minecraft:ui.hud.bubble_pop");
            sound.volume = volume;
            sound.pitch = pitch;
            let _ = host.play_local_sound(&sound, 0);
        }
        Some(frame.draws)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(health: f32, max_health: f32, absorption: f32) -> HealthInputs {
        HealthInputs {
            tick: 1,
            millis: 0,
            health,
            max_health,
            absorption,
            armor: 0,
            food: 20,
            saturation: 5.0,
            kind: HeartKind::Normal,
            regeneration: false,
            hunger: false,
            hardcore: false,
            damage_cooldown: false,
            air: 300,
            max_air: 300,
            under_water: false,
            vehicle_hearts: 0,
        }
    }

    #[test]
    fn health_within_20_is_vanilla_hearts() {
        // 7 health: three full hearts, a half, then empty containers.
        assert_eq!(
            slot(7, 2),
            Slot {
                under: None,
                top: Some((0, false))
            }
        );
        assert_eq!(
            slot(7, 3),
            Slot {
                under: None,
                top: Some((0, true))
            }
        );
        assert_eq!(
            slot(7, 4),
            Slot {
                under: None,
                top: None
            }
        );
    }

    #[test]
    fn health_past_20_colours_the_row_from_the_left() {
        // 45 health: layer 2 (41-60) holds 5 points: two full, a half over a full layer-1 heart,
        // and the rest of the row stays full in layer 1.
        assert_eq!(
            slot(45, 1),
            Slot {
                under: None,
                top: Some((2, false))
            }
        );
        assert_eq!(
            slot(45, 2),
            Slot {
                under: Some(1),
                top: Some((2, true))
            }
        );
        assert_eq!(
            slot(45, 9),
            Slot {
                under: Some(1),
                top: None
            }
        );
        // Exactly 40: the whole row is full layer 1.
        assert_eq!(
            slot(40, 9),
            Slot {
                under: None,
                top: Some((1, false))
            }
        );
    }

    #[test]
    fn layer_colours_cycle() {
        assert_eq!(layer_color(&HEALTH_COLORS, 1), 0xF0_6E_14);
        assert_eq!(layer_color(&HEALTH_COLORS, 8), 0xF0_6E_14);
    }

    #[test]
    fn sixty_health_draws_one_row_with_armour_just_above() {
        let mut i = inputs(60.0, 60.0, 0.0);
        i.armor = 20;
        let frame = State::default().layout(&i, 400, 300, false);
        let containers: Vec<_> = frame
            .draws
            .iter()
            .filter_map(|d| match d {
                GuiDraw::Sprite { sprite, y, .. } if sprite == "hud/heart/container" => Some(*y),
                _ => None,
            })
            .collect();
        // Ten containers, all on the base row (300 - 39); vanilla would stack three rows.
        assert_eq!(containers, vec![261; 10]);
        let armour = frame.draws.iter().find_map(|d| match d {
            GuiDraw::Sprite { sprite, y, .. } if sprite == "hud/armor_full" => Some(*y),
            _ => None,
        });
        assert_eq!(armour, Some(251));
    }

    #[test]
    fn absorption_gets_its_own_row_and_lifts_the_armour() {
        let mut i = inputs(20.0, 20.0, 8.0);
        i.armor = 2;
        let frame = State::default().layout(&i, 400, 300, false);
        let absorbing = frame
            .draws
            .iter()
            .filter(|d| matches!(d, GuiDraw::Sprite { sprite, y: 251, .. } if sprite == "hud/heart/absorbing_full"))
            .count();
        assert_eq!(absorbing, 4);
        let armour = frame.draws.iter().find_map(|d| match d {
            GuiDraw::Sprite { sprite, y, .. } if sprite == "hud/armor_full" => Some(*y),
            _ => None,
        });
        assert_eq!(armour, Some(241));
    }
}
