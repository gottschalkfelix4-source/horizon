//! Smithing Template Viewer for Horizon: hovering an armour trim smithing template shows, in its
//! tooltip, an armour stand wearing armour with that trim, the trim material changing once a
//! second, with the material's name under it.
//!
//! Written from the mod's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). README.md says what still has to be checked against it.
//!
//! Which pattern a template applies is read from the client's smithing trim recipes (the
//! template ingredient and its pattern), falling back to vanilla's naming
//! (`<pattern>_armor_trim_smithing_template`).
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use mod_api::{
    ClientHost, Compound, GuiEntity, GuiEntityLook, GuiEntityView, Hooks, ItemPart, Recipe,
    RecipeRule, RecipeTrimPattern, RecipesReceivedEvent, Registrar, RegistrationContext, Tag,
    TooltipComponentDrawEvent, TooltipComponentEvent,
};

/// Vanilla's trim patterns, whose templates get the preview.
pub const PATTERNS: [&str; 18] = [
    "sentry",
    "dune",
    "coast",
    "wild",
    "ward",
    "eye",
    "vex",
    "tide",
    "snout",
    "rib",
    "spire",
    "wayfinder",
    "shaper",
    "silence",
    "raiser",
    "host",
    "flow",
    "bolt",
];

/// Vanilla's trim materials, in the order the preview cycles through them.
pub const MATERIALS: [&str; 11] = [
    "quartz",
    "iron",
    "netherite",
    "redstone",
    "copper",
    "gold",
    "emerald",
    "diamond",
    "lapis",
    "amethyst",
    "resin",
];

/// The armour the stand wears: iron, so every material but iron shows in its own colour (iron
/// shows its darker variant).
const ARMOUR: [(&str, &str); 4] = [
    ("head", "minecraft:iron_helmet"),
    ("chest", "minecraft:iron_chestplate"),
    ("legs", "minecraft:iron_leggings"),
    ("feet", "minecraft:iron_boots"),
];

/// The preview's size in the tooltip, and the stand's scale (GUI pixels per block).
const WIDTH: f32 = 44.0;
const STAND: f32 = 54.0;
const LINE: f32 = 10.0;
const SCALE: f32 = 26.0;

/// The pattern vanilla's naming gives a template: `minecraft:coast_armor_trim_smithing_template`
/// applies `minecraft:coast`.
pub fn pattern_from_name(template: &str) -> Option<String> {
    let (ns, path) = template.split_once(':')?;
    let pattern = path.strip_suffix("_armor_trim_smithing_template")?;
    (!pattern.is_empty()).then(|| format!("{ns}:{pattern}"))
}

/// Which pattern each template applies, read from smithing trim recipes: (template items,
/// pattern id).
pub type Trims = Vec<(Vec<String>, String)>;

pub fn trims_of(recipes: &[Recipe]) -> Trims {
    recipes
        .iter()
        .filter_map(|r| match &r.rule {
            Some(RecipeRule::SmithingTrim {
                template, pattern, ..
            }) => {
                let pattern = match pattern {
                    RecipeTrimPattern::Reference(id) => id.clone(),
                    RecipeTrimPattern::Inline { asset_id, .. } => asset_id.clone(),
                    _ => return None,
                };
                let mut items = template.order.clone();
                items.extend(
                    template
                        .items
                        .iter()
                        .filter(|i| !template.order.contains(i))
                        .cloned(),
                );
                Some((items, pattern))
            }
            _ => None,
        })
        .collect()
}

/// The pattern `template` applies: a recipe's word first, else vanilla's naming.
pub fn pattern_of(template: &str, trims: &Trims) -> Option<String> {
    trims
        .iter()
        .find(|(items, _)| items.iter().any(|i| i == template))
        .map(|(_, p)| p.clone())
        .or_else(|| pattern_from_name(template))
}

/// The material shown `elapsed_secs` after the game started: one a second, round and round.
pub fn material_at(elapsed_secs: u64) -> &'static str {
    MATERIALS[(elapsed_secs % MATERIALS.len() as u64) as usize]
}

/// An armour stand's saved fields wearing iron armour trimmed with `pattern` in `material`,
/// arms shown and no base plate, as `/summon minecraft:armor_stand ~ ~ ~ {...}` reads them.
pub fn stand_data(pattern: &str, material: &str) -> Compound {
    let trim: Compound = [
        ("pattern".into(), Tag::String(pattern.into())),
        ("material".into(), Tag::String(material.into())),
    ]
    .into();
    let components: Compound = [("minecraft:trim".into(), Tag::Compound(trim))].into();
    let equipment: Compound = ARMOUR
        .iter()
        .map(|(slot, item)| {
            let stack: Compound = [
                ("id".into(), Tag::String((*item).into())),
                ("count".into(), Tag::Int(1)),
                ("components".into(), Tag::Compound(components.clone())),
            ]
            .into();
            ((*slot).into(), Tag::Compound(stack))
        })
        .collect();
    [
        ("equipment".into(), Tag::Compound(equipment)),
        ("ShowArms".into(), Tag::Byte(1)),
        ("NoBasePlate".into(), Tag::Byte(1)),
    ]
    .into()
}

fn started() -> Instant {
    static START: OnceLock<Instant> = OnceLock::new();
    *START.get_or_init(Instant::now)
}

/// The client's trims, read once and again when recipes arrive from a server.
static TRIMS: Mutex<Option<Trims>> = Mutex::new(None);

fn trims(host: &dyn ClientHost) -> Trims {
    let mut cached = TRIMS.lock().unwrap_or_else(|e| e.into_inner());
    cached
        .get_or_insert_with(|| trims_of(&host.recipes_of_type("minecraft:smithing")))
        .clone()
}

fn material_name(host: &dyn ClientHost, material: &str) -> String {
    let key = format!("trim_material.minecraft.{material}");
    host.translate(&key).unwrap_or(key)
}

/// The tooltip preview of the templates.
struct Preview;

#[mod_api::horizon_part]
impl ItemPart for Preview {
    fn tooltip_component(
        &self,
        host: &dyn ClientHost,
        event: &mut TooltipComponentEvent<'_>,
    ) -> Option<[f32; 2]> {
        let template = event.stack.item().id(host);
        pattern_of(&template, &trims(host))?;
        Some([WIDTH, STAND + LINE])
    }

    fn tooltip_component_draw(
        &self,
        host: &dyn ClientHost,
        event: &mut TooltipComponentDrawEvent<'_>,
    ) {
        let template = event.stack.item().id(host);
        let Some(pattern) = pattern_of(&template, &trims(host)) else {
            return;
        };
        let elapsed = started().elapsed();
        let material = material_at(elapsed.as_secs());
        let [x, y] = event.origin;
        let stand = GuiEntity::with_data(
            "minecraft:armor_stand",
            stand_data(&pattern, &format!("minecraft:{material}")),
        );
        // Turning slowly so the back's trim shows too.
        let yaw = 150.0 + (elapsed.as_secs_f32() * 30.0) % 360.0;
        let view = GuiEntityView::new(
            [x, y, x + WIDTH, y + STAND],
            SCALE,
            0.0,
            GuiEntityLook::Turned { yaw, pitch: 0.0 },
        );
        let _ = event.draw.entity(&stand, view);
        let name = material_name(host, material);
        event
            .draw
            .text(&name, [x, y + STAND + 1.0], 0xFFAAAAAA, true);
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for pattern in PATTERNS {
            let item = r.item_key(&format!("minecraft:{pattern}_armor_trim_smithing_template"));
            r.item_part(item, Preview);
        }
    }

    fn recipes_received(&self, _: &mut dyn ClientHost, _: &mut RecipesReceivedEvent<'_>) {
        *TRIMS.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::Ingredient;

    #[test]
    fn vanilla_names_give_the_pattern() {
        assert_eq!(
            pattern_from_name("minecraft:coast_armor_trim_smithing_template").as_deref(),
            Some("minecraft:coast")
        );
        assert_eq!(
            pattern_from_name("minecraft:netherite_upgrade_smithing_template"),
            None
        );
        assert_eq!(
            pattern_from_name("minecraft:_armor_trim_smithing_template"),
            None
        );
    }

    #[test]
    fn a_trim_recipe_names_the_pattern_of_its_template() {
        let mut trim = Recipe::new("mymod:odd_trim", "minecraft:smithing", Default::default());
        trim.rule = Some(RecipeRule::SmithingTrim {
            template: Ingredient::of(vec!["mymod:odd_template".into()]),
            base: Ingredient::of(vec!["minecraft:iron_chestplate".into()]),
            addition: Ingredient::of(vec!["minecraft:gold_ingot".into()]),
            pattern: RecipeTrimPattern::Reference("mymod:spiral".into()),
        });
        let plain = Recipe::new("minecraft:stick", "minecraft:crafting", Default::default());
        let trims = trims_of(&[trim, plain]);
        assert_eq!(
            pattern_of("mymod:odd_template", &trims).as_deref(),
            Some("mymod:spiral")
        );
        // A vanilla template with no recipe read still finds its pattern by name.
        assert_eq!(
            pattern_of("minecraft:wild_armor_trim_smithing_template", &trims).as_deref(),
            Some("minecraft:wild")
        );
        assert_eq!(pattern_of("minecraft:stick", &trims), None);
    }

    #[test]
    fn the_material_changes_each_second_and_comes_round_again() {
        assert_eq!(material_at(0), "quartz");
        assert_eq!(material_at(5), "gold");
        assert_eq!(material_at(11), "quartz");
    }

    #[test]
    fn the_stand_wears_four_trimmed_pieces() {
        let data = stand_data("minecraft:coast", "minecraft:gold");
        let Some(Tag::Compound(equipment)) = data.get(&"equipment".into()) else {
            panic!("no equipment");
        };
        assert_eq!(equipment.len(), 4);
        let Some(Tag::Compound(chest)) = equipment.get(&"chest".into()) else {
            panic!("no chestplate");
        };
        assert_eq!(
            chest.get(&"id".into()),
            Some(&Tag::String("minecraft:iron_chestplate".into()))
        );
        let Some(Tag::Compound(components)) = chest.get(&"components".into()) else {
            panic!("no components");
        };
        let Some(Tag::Compound(trim)) = components.get(&"minecraft:trim".into()) else {
            panic!("no trim");
        };
        assert_eq!(
            trim.get(&"pattern".into()),
            Some(&Tag::String("minecraft:coast".into()))
        );
        assert_eq!(
            trim.get(&"material".into()),
            Some(&Tag::String("minecraft:gold".into()))
        );
    }
}
