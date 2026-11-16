//! AppleSkin for Horizon: food values in item tooltips, the saturation and exhaustion the
//! hunger bar hides, and a flashing preview of what the held food will restore.
//!
//! Written from AppleSkin's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and what a comparison with the
//! NeoForge original still has to confirm, is in README.md.
//!
//! Everything that decides a number (icons in a tooltip, the outline on each shank, the food
//! and saturation after eating, the health natural regeneration will give back) is a pure
//! function below with its own test; the hooks only read the player and draw.
use std::borrow::Cow;

use mod_api::{
    ClientHost, EntityRef, GuiDraw, GuiLayerPostEvent, GuiLayerPreEvent, Hooks, ItemStack,
    ItemTooltipEvent, PlayerRef, Registrar, RegistrationContext, SettingSpec, Tag, Text,
    client::hud_layout::Blend,
    text::{TextContent, TextObject, TextStyle},
};

/// The mod id, as settings and translation keys use it.
pub const MOD_ID: &str = "appleskin";

/// Food the most a player holds (`FoodConstants.MAX_FOOD`).
pub const MAX_FOOD: i32 = 20;

/// Exhaustion at which one point of saturation (or food) is spent (`FoodData.tick`).
pub const EXHAUSTION_PER_POINT: f32 = 4.0;

/// The HUD layer the health, armour, food and air rows are drawn in.
const HEALTH_LAYER: &str = "minecraft:player_health";

/// What eating an item gives: its `minecraft:food` component.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FoodValues {
    /// Food points (half shanks).
    pub nutrition: i32,
    /// Saturation points added (already `nutrition * modifier * 2`, as 1.21 stores it).
    pub saturation: f32,
    /// Whether it can be eaten with a full bar (golden apples, chorus fruit...).
    pub always_edible: bool,
}

/// The player's food state (`FoodData`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Food {
    pub level: i32,
    pub saturation: f32,
    pub exhaustion: f32,
}

impl Food {
    /// `FoodData.eat(nutrition, saturation)`: food is capped at 20, saturation at the new food level.
    pub fn after_eating(self, food: FoodValues) -> Food {
        let level = (self.level + food.nutrition).clamp(0, MAX_FOOD);
        let saturation = (self.saturation + food.saturation).clamp(0.0, level as f32);
        Food {
            level,
            saturation,
            exhaustion: self.exhaustion,
        }
    }
}

/// How one shank-sized slot is filled: full, half or empty, for food points `points` at slot `i`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fill {
    Empty,
    Half,
    Full,
}

/// Slot `i` (0 = rightmost, the first drawn) of a bar showing `points` half-icons.
pub fn fill_at(points: i32, i: i32) -> Fill {
    let left = points - i * 2;
    if left >= 2 {
        Fill::Full
    } else if left == 1 {
        Fill::Half
    } else {
        Fill::Empty
    }
}

/// Quarters of the saturation outline drawn over food slot `i` (0..=4): a whole outline per two
/// points of saturation, then a part in steps of half a point, as the overlay shows them.
pub fn saturation_quarters(saturation: f32, i: i32) -> u32 {
    let left = saturation - i as f32 * 2.0;
    if left <= 0.0 {
        0
    } else if left >= 2.0 {
        4
    } else {
        // 0.5 → 1 quarter, 1.0 → 2, 1.5 → 3; a fraction below the step rounds up so a little
        // saturation is still visible.
        ((left * 2.0).ceil() as u32).clamp(1, 4)
    }
}

/// The icons a tooltip shows for `food`: full and half hunger shanks, and saturation in shanks
/// (two points each, rounded to a tenth as the numbers beside them are).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TooltipIcons {
    pub full_shanks: i32,
    pub half_shank: bool,
    pub saturation_shanks: f32,
}

pub fn tooltip_icons(food: FoodValues) -> TooltipIcons {
    let nutrition = food.nutrition.max(0);
    TooltipIcons {
        full_shanks: nutrition / 2,
        half_shank: nutrition % 2 == 1,
        saturation_shanks: (food.saturation / 2.0 * 10.0).round() / 10.0,
    }
}

/// The health natural regeneration gives back from `food` before it runs out or `health`
/// reaches `max_health`, simulating `FoodData.tick` tick by tick: a point of exhaustion over 4
/// spends saturation, then food; with a full bar and saturation, every 10 ticks heals
/// `min(saturation, 6) / 6` for as much exhaustion; with food 18 or more, every 80 ticks heals 1
/// for 6 exhaustion.
pub fn regen_estimate(food: Food, health: f32, max_health: f32) -> f32 {
    let Food {
        mut level,
        mut saturation,
        mut exhaustion,
    } = food;
    let mut hp = health;
    let mut timer = 0;
    // Twenty minutes of ticks is far more than any bar can last; the cap only guards the loop.
    for _ in 0..24_000 {
        if hp >= max_health {
            break;
        }
        if exhaustion > EXHAUSTION_PER_POINT {
            exhaustion -= EXHAUSTION_PER_POINT;
            if saturation > 0.0 {
                saturation = (saturation - 1.0).max(0.0);
            } else {
                level = (level - 1).max(0);
            }
        }
        if saturation > 0.0 && level >= MAX_FOOD {
            timer += 1;
            if timer >= 10 {
                let spent = saturation.min(6.0);
                hp = (hp + spent / 6.0).min(max_health);
                exhaustion = (exhaustion + spent).min(40.0);
                timer = 0;
            }
        } else if level >= 18 {
            timer += 1;
            if timer >= 80 {
                hp = (hp + 1.0).min(max_health);
                exhaustion = (exhaustion + 6.0).min(40.0);
                timer = 0;
            }
        } else {
            break;
        }
    }
    hp - health
}

/// Width in GUI pixels of the exhaustion underlay behind the 81-pixel food row.
pub fn exhaustion_width(exhaustion: f32) -> i32 {
    let ratio = (exhaustion / EXHAUSTION_PER_POINT).clamp(0.0, 1.0);
    (ratio * 81.0) as i32
}

/// Alpha of the flashing preview icons, 0.25 to 0.75, one cycle a second and a half.
pub fn flash_alpha(millis: u64) -> f32 {
    let phase = (millis % 1500) as f32 / 1500.0 * std::f32::consts::TAU;
    0.5 + 0.25 * phase.sin()
}

/// Rows of the health bar as vanilla lays them (`Gui.renderPlayerHealth`): how many heart rows
/// there are and how far apart they sit, for health `max_health` with `absorption`.
pub fn heart_rows(max_health: f32, absorption: f32) -> (i32, i32) {
    let absorption = absorption.ceil();
    let rows = ((max_health + absorption) / 2.0 / 10.0).ceil().max(1.0) as i32;
    let row_height = (10 - (rows - 2)).max(3);
    (rows, row_height)
}

// ---------------------------------------------------------------------------------------------
// Reading the game.

/// Reads a whole or decimal number from an NBT value, as an item report stores it.
fn number(tag: &Tag) -> Option<f64> {
    Some(match tag {
        Tag::Byte(v) => f64::from(*v),
        Tag::Short(v) => f64::from(*v),
        Tag::Int(v) => f64::from(*v),
        Tag::Long(v) => *v as f64,
        Tag::Float(bits) => f64::from(f32::from_bits(*bits)),
        Tag::Double(bits) => f64::from_bits(*bits),
        _ => return None,
    })
}

/// Parses a `minecraft:food` component value.
pub fn food_from_tag(tag: &Tag) -> Option<FoodValues> {
    let Tag::Compound(c) = tag else {
        return None;
    };
    let nutrition = number(c.get(&"nutrition".into())?)? as i32;
    let saturation = c.get(&"saturation".into()).and_then(number).unwrap_or(0.0) as f32;
    let always_edible = c
        .get(&"can_always_eat".into())
        .and_then(number)
        .is_some_and(|v| v != 0.0);
    Some(FoodValues {
        nutrition,
        saturation,
        always_edible,
    })
}

/// The stack's food: its own component (or its removal) first, else its item's default.
fn stack_food(host: &dyn ClientHost, stack: &ItemStack) -> Option<FoodValues> {
    if let Some(patch) = stack.components() {
        let patch = patch.as_nbt();
        if patch.contains_key(&"!minecraft:food".into()) {
            return None;
        }
        if let Some(tag) = patch.get(&"minecraft:food".into()) {
            return food_from_tag(tag);
        }
    }
    food_from_tag(&host.item_default_component(stack.item(), "minecraft:food")?)
}

fn setting(host: &dyn ClientHost, key: &str) -> bool {
    host.setting_bool(key).unwrap_or(true)
}

/// What the HUD overlay reads of the local player each frame.
struct Player {
    food: Food,
    health: f32,
    max_health: f32,
    absorption: f32,
    hunger: bool,
    held: Option<FoodValues>,
}

fn local_player(host: &dyn ClientHost) -> Option<Player> {
    let player: PlayerRef = host.local_player()?;
    if player.is_creative(host) == Some(true) || player.is_spectator(host) == Some(true) {
        return None;
    }
    let food = player.food(host)?;
    let living = host.living(EntityRef::from(player))?;
    let hunger = host
        .effect_key("minecraft:hunger")
        .and_then(|effect| living.effect(host, effect))
        .is_some();
    let held = player.selected_slot(host).and_then(|slot| {
        let slots = player.inventory_slots(host)?;
        let (_, stack) = slots.iter().find(|(s, _)| *s as usize == slot)?;
        stack_food(host, stack)
    });
    Some(Player {
        food: Food {
            level: food.level,
            saturation: food.saturation,
            exhaustion: food.exhaustion,
        },
        health: living.health(host)?,
        max_health: living.max_health(host)?,
        absorption: living.absorption(host).unwrap_or(0.0),
        hunger,
        held,
    })
}

// ---------------------------------------------------------------------------------------------
// Drawing.

/// AppleSkin's saturation gold.
const SATURATION_ARGB: u32 = 0xFF_FF_C8_2D;

fn sprite(name: &'static str, x: i32, y: i32, alpha: f32) -> GuiDraw {
    let a = (alpha.clamp(0.0, 1.0) * 255.0) as u32;
    GuiDraw::Sprite {
        sprite: Cow::Borrowed(name),
        x,
        y,
        w: 9,
        h: 9,
        region: None,
        color: (a << 24) | 0x00FF_FFFF,
        blend: Blend::Textured,
    }
}

fn fill(out: &mut Vec<GuiDraw>, x0: i32, y0: i32, x1: i32, y1: i32, color: u32) {
    out.push(GuiDraw::Fill {
        x0,
        y0,
        x1,
        y1,
        color,
    });
}

/// A saturation outline round the 9x9 shank at `(x, y)`, `quarters` of it, from the right
/// edge round the bottom (the overlay's look; drawn with fills, not AppleSkin's texture).
fn saturation_outline(out: &mut Vec<GuiDraw>, x: i32, y: i32, quarters: u32, color: u32) {
    if quarters >= 1 {
        fill(out, x + 8, y, x + 9, y + 9, color); // right
    }
    if quarters >= 2 {
        fill(out, x, y + 8, x + 9, y + 9, color); // bottom
    }
    if quarters >= 3 {
        fill(out, x, y, x + 1, y + 9, color); // left
    }
    if quarters >= 4 {
        fill(out, x, y, x + 9, y + 1, color); // top
    }
}

fn food_sprites(hunger: bool) -> (&'static str, &'static str) {
    if hunger {
        ("hud/food_full_hunger", "hud/food_half_hunger")
    } else {
        ("hud/food_full", "hud/food_half")
    }
}

/// The overlay over the food and health rows, for a GUI of `w` x `h`.
fn overlay(p: &Player, w: i32, h: i32, alpha: f32, show: Shown) -> Vec<GuiDraw> {
    let mut out = Vec::new();
    let (left, right, top) = (w / 2 - 91, w / 2 + 91, h - 39);
    let slot_x = |i: i32| right - i * 8 - 9;
    if show.saturation {
        for i in 0..10 {
            let q = saturation_quarters(p.food.saturation, i);
            if q > 0 {
                saturation_outline(&mut out, slot_x(i), top, q, SATURATION_ARGB);
            }
        }
    }
    let Some(held) = p.held else {
        return out;
    };
    if p.food.level >= MAX_FOOD && !held.always_edible {
        return out;
    }
    let after = p.food.after_eating(held);
    if show.held_food {
        let (full, half) = food_sprites(p.hunger);
        for i in 0..10 {
            let now = fill_at(p.food.level, i);
            let then = fill_at(after.level, i);
            if then != now && then != Fill::Empty {
                let name = if then == Fill::Full { full } else { half };
                out.push(sprite(name, slot_x(i), top, alpha));
            }
        }
        if show.saturation {
            let faded = ((alpha * 255.0) as u32) << 24 | (SATURATION_ARGB & 0x00FF_FFFF);
            for i in 0..10 {
                let q = saturation_quarters(after.saturation, i);
                if q > saturation_quarters(p.food.saturation, i) {
                    saturation_outline(&mut out, slot_x(i), top, q, faded);
                }
            }
        }
    }
    if show.health {
        let gained = regen_estimate(after, p.health, p.max_health);
        let max = p.max_health.max(p.health);
        let (_, row_height) = heart_rows(max, p.absorption);
        let now = p.health.ceil() as i32;
        let then = (p.health + gained).ceil().min(max) as i32;
        for c in 0..(max / 2.0).ceil() as i32 {
            let shown_now = fill_at(now, c);
            let shown_then = fill_at(then, c);
            if shown_then != shown_now && shown_then != Fill::Empty {
                let name = if shown_then == Fill::Full {
                    "hud/heart/full"
                } else {
                    "hud/heart/half"
                };
                let x = left + (c % 10) * 8;
                let y = top - (c / 10) * row_height;
                out.push(sprite(name, x, y, alpha));
            }
        }
    }
    out
}

#[derive(Clone, Copy)]
struct Shown {
    saturation: bool,
    held_food: bool,
    health: bool,
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for (key, label) in [
            ("show_food_values_in_tooltip", "appleskin.config.tooltip"),
            ("show_saturation_hud_overlay", "appleskin.config.saturation"),
            ("show_food_values_hud_overlay", "appleskin.config.held_food"),
            ("show_food_health_hud_overlay", "appleskin.config.health"),
            (
                "show_food_exhaustion_hud_underlay",
                "appleskin.config.exhaustion",
            ),
        ] {
            let _ = r.register_setting(
                SettingSpec::toggle(key, label, true).tooltip(format!("{label}.tooltip")),
            );
        }
    }

    /// Hunger shanks and saturation under the item's lines.
    fn item_tooltip(&self, host: &dyn ClientHost, event: &mut ItemTooltipEvent<'_>) {
        if !host.on(MOD_ID) || !setting(host, "show_food_values_in_tooltip") {
            return;
        }
        let Some(food) = stack_food(host, event.stack) else {
            return;
        };
        let icons = tooltip_icons(food);
        let shank = |name: &str| {
            Text::new(TextContent::Object(TextObject::Atlas {
                atlas: "minecraft:gui".into(),
                sprite: name.into(),
                fallback: None,
            }))
        };
        let mut hunger = Text::literal("");
        if icons.full_shanks > 10 {
            hunger = hunger
                .append(shank("hud/food_full"))
                .append(Text::translate(
                    "appleskin.tooltip.times",
                    vec![Text::literal(icons.full_shanks.to_string())],
                ));
        } else {
            for _ in 0..icons.full_shanks {
                hunger = hunger.append(shank("hud/food_full"));
            }
        }
        if icons.half_shank {
            hunger = hunger.append(shank("hud/food_half"));
        }
        event.lines.push((hunger, u32::MAX));
        let mut gold = TextStyle::default();
        gold.color = Some("gold".into());
        let saturation = Text::translate(
            "appleskin.tooltip.saturation",
            vec![Text::literal(format!(
                "{:.1}",
                icons.saturation_shanks * 2.0
            ))],
        )
        .with_style(gold);
        event.lines.push((saturation, u32::MAX));
    }

    /// The exhaustion underlay goes under the food row.
    fn gui_layer_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut GuiLayerPreEvent<'_>,
    ) -> mod_api::Flow {
        if event.layer != HEALTH_LAYER
            || !host.on(MOD_ID)
            || !setting(host, "show_food_exhaustion_hud_underlay")
        {
            return mod_api::Flow::Continue;
        }
        if let Some(p) = local_player(host) {
            let [w, h] = event.draw.size();
            let (w, h) = (w as i32, h as i32);
            let width = exhaustion_width(p.food.exhaustion);
            if width > 0 {
                let right = w / 2 + 91;
                let top = h - 39;
                event.draw.fill(
                    [(right - width) as f32, top as f32, width as f32, 9.0],
                    0x60_FF_FF_FF,
                );
            }
        }
        mod_api::Flow::Continue
    }

    /// Saturation, the held food's restore and the health it will give back, over the rows.
    fn gui_layer_post(&self, host: &dyn ClientHost, event: &mut GuiLayerPostEvent<'_>) {
        if event.layer != HEALTH_LAYER || !host.on(MOD_ID) {
            return;
        }
        let Some(p) = local_player(host) else {
            return;
        };
        let shown = Shown {
            saturation: setting(host, "show_saturation_hud_overlay"),
            held_food: setting(host, "show_food_values_hud_overlay"),
            health: setting(host, "show_food_health_hud_overlay"),
        };
        let [w, h] = event.draw.size();
        let draws = overlay(&p, w as i32, h as i32, flash_alpha(host.millis()), shown);
        if !draws.is_empty() {
            let _ = event.draw.gui_paint(&draws);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEAK: FoodValues = FoodValues {
        nutrition: 8,
        saturation: 12.8,
        always_edible: false,
    };

    #[test]
    fn a_steak_shows_four_shanks_and_six_point_four_saturation_shanks() {
        let icons = tooltip_icons(STEAK);
        assert_eq!(icons.full_shanks, 4);
        assert!(!icons.half_shank);
        assert_eq!(icons.saturation_shanks, 6.4);
    }

    #[test]
    fn bread_shows_a_half_shank() {
        // Bread: 5 nutrition, 6.0 saturation.
        let icons = tooltip_icons(FoodValues {
            nutrition: 5,
            saturation: 6.0,
            always_edible: false,
        });
        assert_eq!((icons.full_shanks, icons.half_shank), (2, true));
    }

    #[test]
    fn eating_caps_food_at_20_and_saturation_at_the_new_food_level() {
        let food = Food {
            level: 15,
            saturation: 2.0,
            exhaustion: 0.0,
        };
        let after = food.after_eating(STEAK);
        assert_eq!(after.level, 20);
        assert!((after.saturation - 14.8).abs() < 1e-4);
        let starving = Food {
            level: 2,
            saturation: 0.0,
            exhaustion: 0.0,
        };
        // 2 + 8 = 10 food; 12.8 saturation is cut to 10.
        assert_eq!(starving.after_eating(STEAK).saturation, 10.0);
    }

    #[test]
    fn saturation_outlines_fill_two_points_per_shank_then_quarters() {
        // 5 saturation: two whole outlines, then one point (half an outline), then none.
        assert_eq!(saturation_quarters(5.0, 0), 4);
        assert_eq!(saturation_quarters(5.0, 1), 4);
        assert_eq!(saturation_quarters(5.0, 2), 2);
        assert_eq!(saturation_quarters(5.0, 3), 0);
        assert_eq!(saturation_quarters(0.5, 0), 1);
    }

    #[test]
    fn a_full_bar_without_saturation_gives_back_three_health() {
        // Food 20, no saturation: a heal every 80 ticks costs 6 exhaustion, and each point over
        // 4 spends a food point, one per tick. Heals at food 20, 19→18 and 18; then food is 16.
        let food = Food {
            level: 20,
            saturation: 0.0,
            exhaustion: 0.0,
        };
        assert_eq!(regen_estimate(food, 10.0, 20.0), 3.0);
    }

    #[test]
    fn regeneration_stops_at_max_health() {
        let food = Food {
            level: 20,
            saturation: 20.0,
            exhaustion: 0.0,
        };
        assert_eq!(regen_estimate(food, 19.0, 20.0), 1.0);
        assert_eq!(regen_estimate(food, 20.0, 20.0), 0.0);
    }

    #[test]
    fn exhaustion_underlay_grows_with_exhaustion() {
        assert_eq!(exhaustion_width(0.0), 0);
        assert_eq!(exhaustion_width(2.0), 40);
        assert_eq!(exhaustion_width(9.0), 81);
    }

    #[test]
    fn the_food_component_is_read_from_its_report_form() {
        let tag = Tag::Compound(
            [
                ("nutrition".into(), Tag::Int(4)),
                ("saturation".into(), Tag::Float(9.6f32.to_bits())),
                ("can_always_eat".into(), Tag::Byte(1)),
            ]
            .into(),
        );
        assert_eq!(
            food_from_tag(&tag),
            Some(FoodValues {
                nutrition: 4,
                saturation: 9.6,
                always_edible: true
            })
        );
    }

    #[test]
    fn held_food_flashes_only_the_shanks_it_adds() {
        let p = Player {
            food: Food {
                level: 14,
                saturation: 0.0,
                exhaustion: 0.0,
            },
            health: 20.0,
            max_health: 20.0,
            absorption: 0.0,
            hunger: false,
            held: Some(FoodValues {
                nutrition: 3,
                saturation: 0.0,
                always_edible: false,
            }),
        };
        let draws = overlay(
            &p,
            400,
            300,
            0.5,
            Shown {
                saturation: false,
                held_food: true,
                health: true,
            },
        );
        // 14 → 17: slot 7 (x = 200 + 91 - 7 * 8 - 9 = 226) becomes full, slot 8 (x = 218) half.
        let sprites: Vec<_> = draws
            .iter()
            .filter_map(|d| match d {
                GuiDraw::Sprite { sprite, x, .. } => Some((sprite.to_string(), *x)),
                _ => None,
            })
            .collect();
        assert_eq!(
            sprites,
            vec![
                ("hud/food_full".to_string(), 226),
                ("hud/food_half".to_string(), 218)
            ]
        );
    }
}
