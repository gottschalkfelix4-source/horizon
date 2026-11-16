//! JEI's own plugin for vanilla's recipes (`VanillaPlugin`): crafting, smelting, blasting,
//! smoking, campfire cooking, stonecutting, smithing, fuel and composting, with the blocks
//! that make each.
use mod_api::{Host, ItemStack, Recipe, RecipeRule};

use crate::plugin::{Category, Display, JeiPlugin, Registration, Role};

/// Category uids. The recipe types keep their own id; fuel and composting are JEI's.
pub const CRAFTING: &str = "minecraft:crafting";
pub const SMELTING: &str = "minecraft:smelting";
pub const BLASTING: &str = "minecraft:blasting";
pub const SMOKING: &str = "minecraft:smoking";
pub const CAMPFIRE: &str = "minecraft:campfire_cooking";
pub const STONECUTTING: &str = "minecraft:stonecutting";
pub const SMITHING: &str = "minecraft:smithing";
pub const FUEL: &str = "jei:fuel";
pub const COMPOSTING: &str = "jei:compostable";

const RECIPE_TYPES: [&str; 7] = [
    CRAFTING,
    SMELTING,
    BLASTING,
    SMOKING,
    CAMPFIRE,
    STONECUTTING,
    SMITHING,
];

/// The categories in JEI's tab order, with the tab's icon and each recipe's size.
pub fn categories() -> Vec<Category> {
    vec![
        Category::new(
            CRAFTING,
            "gui.jei.category.craftingTable",
            "minecraft:crafting_table",
            116,
            54,
        ),
        Category::new(
            STONECUTTING,
            "gui.jei.category.stoneCutter",
            "minecraft:stonecutter",
            82,
            34,
        ),
        Category::new(
            SMELTING,
            "gui.jei.category.smelting",
            "minecraft:furnace",
            82,
            54,
        ),
        Category::new(
            SMOKING,
            "gui.jei.category.smoking",
            "minecraft:smoker",
            82,
            54,
        ),
        Category::new(
            BLASTING,
            "gui.jei.category.blasting",
            "minecraft:blast_furnace",
            82,
            54,
        ),
        Category::new(
            CAMPFIRE,
            "gui.jei.category.campfire",
            "minecraft:campfire",
            82,
            44,
        ),
        Category::new(
            SMITHING,
            "gui.jei.category.smithing",
            "minecraft:smithing_table",
            108,
            28,
        ),
        Category::new(
            COMPOSTING,
            "gui.jei.category.compostable",
            "minecraft:composter",
            82,
            28,
        ),
        Category::new(FUEL, "gui.jei.category.fuel", "minecraft:furnace", 82, 34),
    ]
}

/// The blocks that make each category's recipes.
pub const CATALYSTS: &[(&str, &str)] = &[
    (CRAFTING, "minecraft:crafting_table"),
    (CRAFTING, "minecraft:crafter"),
    (STONECUTTING, "minecraft:stonecutter"),
    (SMELTING, "minecraft:furnace"),
    (SMOKING, "minecraft:smoker"),
    (BLASTING, "minecraft:blast_furnace"),
    (CAMPFIRE, "minecraft:campfire"),
    (CAMPFIRE, "minecraft:soul_campfire"),
    (SMITHING, "minecraft:smithing_table"),
    (COMPOSTING, "minecraft:composter"),
    (FUEL, "minecraft:furnace"),
    (FUEL, "minecraft:smoker"),
    (FUEL, "minecraft:blast_furnace"),
];

/// A recipe reduced to what JEI draws, read from the host's [`Recipe`] (`parts_of`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Parts {
    pub id: String,
    pub kind: String,
    /// Each ingredient's items, in recipe order (a shaped recipe's filled cells, row by row).
    pub inputs: Vec<Vec<String>>,
    /// The result's items and count.
    pub output: Option<(Vec<String>, u32)>,
    /// A shaped recipe's rows; `None` for shapeless.
    pub pattern: Option<Vec<String>>,
    pub cooking_time: Option<u32>,
    pub experience: Option<f32>,
    /// A trim's result is the base with a trim on it: drawn, but no "R" finds it.
    pub output_render_only: bool,
}

/// `12.0` as `12`, `0.75` as `0.75`: at most two decimals and no trailing zeros, as JEI's
/// `#.##` format prints times and counts.
pub fn number(value: f32) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Where a crafting recipe's `n`th ingredient goes in the 3×3 grid: a shaped recipe's
/// pattern places its filled cells, a shapeless one fills the grid row by row.
fn grid_cells(parts: &Parts) -> Vec<usize> {
    match &parts.pattern {
        Some(rows) => rows
            .iter()
            .take(3)
            .enumerate()
            .flat_map(|(y, row)| {
                row.chars()
                    .take(3)
                    .enumerate()
                    .filter(|(_, c)| *c != ' ')
                    .map(move |(x, _)| y * 3 + x)
            })
            .collect(),
        None => (0..parts.inputs.len().min(9)).collect(),
    }
}

/// The recipe as JEI shows it, and its category; None for a type this plugin doesn't draw or a
/// recipe with nothing to show.
pub fn display(parts: &Parts) -> Option<(&'static str, Display)> {
    let id = Some(parts.id.clone());
    let output_role = if parts.output_render_only {
        Role::RenderOnly
    } else {
        Role::Output
    };
    let output = |d: Display, pos: [i16; 2]| match &parts.output {
        Some((items, count)) => d.slot_counted(output_role, pos, items.clone(), *count),
        None => d,
    };
    let input = |n: usize| parts.inputs.get(n).cloned().unwrap_or_default();
    match parts.kind.as_str() {
        CRAFTING => {
            parts.output.as_ref()?;
            let mut d = Display::new(id).arrow([61, 19]);
            for (n, cell) in grid_cells(parts).into_iter().enumerate() {
                let items = input(n);
                if !items.is_empty() {
                    d = d.input(
                        [(cell % 3) as i16 * 18 + 1, (cell / 3) as i16 * 18 + 1],
                        items,
                    );
                }
            }
            Some((CRAFTING, output(d, [95, 19])))
        }
        kind @ (SMELTING | BLASTING | SMOKING | CAMPFIRE) => {
            let uid = RECIPE_TYPES.into_iter().find(|t| *t == kind)?;
            let mut d = output(
                Display::new(id).input([1, 1], input(0)).arrow([24, 18]),
                [61, 19],
            );
            if let Some(ticks) = parts.cooking_time {
                d = d.line(
                    [0, 45],
                    "gui.jei.category.smelting.time.seconds",
                    vec![number(ticks as f32 / 20.0)],
                );
            }
            if let Some(xp) = parts.experience.filter(|xp| *xp > 0.0) {
                d = d.line(
                    [0, 36],
                    "gui.jei.category.smelting.experience",
                    vec![number(xp)],
                );
            }
            Some((uid, d))
        }
        STONECUTTING => Some((
            STONECUTTING,
            output(
                Display::new(id).input([1, 9], input(0)).arrow([24, 8]),
                [61, 9],
            ),
        )),
        SMITHING => {
            let mut d = Display::new(id).arrow([61, 1]);
            for (n, x) in [1, 19, 37].into_iter().enumerate() {
                let items = input(n);
                if !items.is_empty() {
                    d = d.input([x, 1], items);
                }
            }
            Some((SMITHING, output(d, [91, 1])))
        }
        _ => None,
    }
}

/// A fuel's recipe: the item and how many items it smelts (`burn ticks / 200`).
pub fn fuel(item: &str, burn_ticks: i32) -> Option<Display> {
    (burn_ticks > 0).then(|| {
        Display::new(None)
            .slot(Role::Input, [1, 17], vec![item.to_owned()])
            .line(
                [24, 13],
                "gui.jei.category.fuel.smeltCount",
                vec![number(burn_ticks as f32 / 200.0)],
            )
    })
}

/// A compostable item's recipe: the item and its chance to raise the composter a level.
pub fn compost(item: &str, chance: f32) -> Option<Display> {
    (chance > 0.0).then(|| {
        Display::new(None)
            .slot(Role::Input, [1, 6], vec![item.to_owned()])
            .line(
                [24, 10],
                "gui.jei.category.compostable.chance",
                vec![number(chance * 100.0)],
            )
    })
}

fn ingredient_items(i: &mod_api::Ingredient) -> Vec<String> {
    if i.order.is_empty() {
        let mut items: Vec<String> = i.items.iter().cloned().collect();
        items.sort();
        items
    } else {
        i.order.clone()
    }
}

fn stack_out(host: &dyn Host, stack: &ItemStack) -> (Vec<String>, u32) {
    (vec![stack.item().id(host)], stack.count())
}

/// Read a host recipe into [`Parts`].
pub fn parts_of(host: &dyn Host, recipe: &Recipe) -> Parts {
    let mut parts = Parts {
        id: recipe.id.clone(),
        kind: recipe.kind.clone(),
        inputs: recipe.ingredients.iter().map(ingredient_items).collect(),
        output: recipe.results.first().map(|s| stack_out(host, s)),
        pattern: recipe.json.get("pattern").and_then(|p| {
            p.as_array().map(|rows| {
                rows.iter()
                    .filter_map(|r| r.as_str().map(str::to_owned))
                    .collect()
            })
        }),
        cooking_time: recipe.cooking_time,
        experience: recipe.experience,
        output_render_only: false,
    };
    match &recipe.rule {
        Some(RecipeRule::Stonecutting { input, result }) => {
            parts.inputs = vec![ingredient_items(input)];
            parts.output = Some(stack_out(host, result));
        }
        Some(RecipeRule::SmithingTransform {
            template,
            base,
            addition,
            result,
        }) => {
            parts.inputs = vec![
                template.as_ref().map(ingredient_items).unwrap_or_default(),
                ingredient_items(base),
                addition.as_ref().map(ingredient_items).unwrap_or_default(),
            ];
            parts.output = result
                .item
                .clone()
                .map(|item| (vec![item], result.count.max(1)))
                .or_else(|| Some((ingredient_items(base), 1)));
        }
        Some(RecipeRule::SmithingTrim {
            template,
            base,
            addition,
            ..
        }) => {
            parts.inputs = vec![
                ingredient_items(template),
                ingredient_items(base),
                ingredient_items(addition),
            ];
            parts.output = Some((ingredient_items(base), 1));
            parts.output_render_only = true;
        }
        // Special crafting (dyeing, fireworks, map copying...) assembles from the grid: JEI shows
        // those as fixed examples, not yet here.
        Some(_) => parts.output = None,
        None => {}
    }
    parts
}

/// The built-in plugin.
pub struct VanillaPlugin;

impl JeiPlugin for VanillaPlugin {
    fn uid(&self) -> &str {
        "jei:minecraft"
    }
    fn register_categories(&self, _: &dyn Host, r: &mut Registration) {
        for c in categories() {
            r.add_category(c);
        }
    }
    fn register_recipes(&self, host: &dyn Host, r: &mut Registration) {
        for kind in RECIPE_TYPES {
            for recipe in host.recipes_of_type(kind).iter() {
                if let Some((uid, d)) = display(&parts_of(host, recipe)) {
                    r.add_recipe(uid, d);
                }
            }
        }
        for key in host.item_keys() {
            let id = key.id(host);
            if let Some(d) = host.item_burn_time(key).and_then(|t| fuel(&id, t)) {
                r.add_recipe(FUEL, d);
            }
            if let Some(d) = host.item_compost_chance(key).and_then(|c| compost(&id, c)) {
                r.add_recipe(COMPOSTING, d);
            }
        }
    }
    fn register_catalysts(&self, _: &dyn Host, r: &mut Registration) {
        for (uid, item) in CATALYSTS {
            r.add_catalyst(uid, item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn slot_positions(d: &Display, role: Role) -> Vec<[i16; 2]> {
        d.slots
            .iter()
            .filter(|s| s.role == role)
            .map(|s| s.pos)
            .collect()
    }

    #[test]
    fn a_shaped_recipe_keeps_its_pattern_in_the_grid() {
        // A stick: two planks in the middle column... as vanilla's "#","#" pattern, one wide.
        let parts = Parts {
            id: "minecraft:stick".into(),
            kind: CRAFTING.into(),
            inputs: vec![v(&["minecraft:oak_planks"]), v(&["minecraft:oak_planks"])],
            output: Some((v(&["minecraft:stick"]), 4)),
            pattern: Some(v(&["#", "#"])),
            ..Parts::default()
        };
        let (uid, d) = display(&parts).unwrap();
        assert_eq!(uid, CRAFTING);
        assert_eq!(slot_positions(&d, Role::Input), [[1, 1], [1, 19]]);
        assert_eq!(d.slots.last().unwrap().count, 4);
        // A torch-like " #"/" #" pattern starts in the second column.
        let offset = Parts {
            pattern: Some(v(&[" #", " #"])),
            ..parts
        };
        assert_eq!(
            slot_positions(&display(&offset).unwrap().1, Role::Input),
            [[19, 1], [19, 19]]
        );
    }

    #[test]
    fn a_shapeless_recipe_fills_the_grid_row_by_row() {
        let parts = Parts {
            id: "x:y".into(),
            kind: CRAFTING.into(),
            inputs: vec![v(&["a:a"]); 4],
            output: Some((v(&["a:b"]), 1)),
            pattern: None,
            ..Parts::default()
        };
        let d = display(&parts).unwrap().1;
        assert_eq!(
            slot_positions(&d, Role::Input),
            [[1, 1], [19, 1], [37, 1], [1, 19]]
        );
    }

    #[test]
    fn cooking_shows_seconds_and_experience() {
        // Iron ore in a furnace: 200 ticks, 0.7 experience; in a blast furnace 100 ticks.
        let parts = Parts {
            id: "minecraft:iron_ingot_from_smelting_iron_ore".into(),
            kind: SMELTING.into(),
            inputs: vec![v(&["minecraft:iron_ore"])],
            output: Some((v(&["minecraft:iron_ingot"]), 1)),
            cooking_time: Some(200),
            experience: Some(0.7),
            ..Parts::default()
        };
        let d = display(&parts).unwrap().1;
        let args: Vec<_> = d.lines.iter().map(|l| l.args[0].as_str()).collect();
        assert_eq!(args, ["10", "0.7"]);
        let blasting = Parts {
            kind: BLASTING.into(),
            cooking_time: Some(100),
            ..parts
        };
        let (uid, d) = display(&blasting).unwrap();
        assert_eq!((uid, d.lines[0].args[0].as_str()), (BLASTING, "5"));
    }

    #[test]
    fn coal_smelts_eight_items_and_a_blaze_rod_twelve() {
        assert_eq!(fuel("minecraft:coal", 1600).unwrap().lines[0].args, ["8"]);
        assert_eq!(
            fuel("minecraft:blaze_rod", 2400).unwrap().lines[0].args,
            ["12"]
        );
        // A wooden slab: 150 ticks.
        assert_eq!(
            fuel("minecraft:oak_slab", 150).unwrap().lines[0].args,
            ["0.75"]
        );
        assert!(fuel("minecraft:stone", 0).is_none());
    }

    #[test]
    fn composting_shows_the_chance_in_percent() {
        assert_eq!(
            compost("minecraft:wheat_seeds", 0.3).unwrap().lines[0].args,
            ["30"]
        );
        assert_eq!(
            compost("minecraft:cake", 1.0).unwrap().lines[0].args,
            ["100"]
        );
    }

    #[test]
    fn a_trims_result_is_drawn_but_not_a_recipe_for_the_armour() {
        let parts = Parts {
            id: "minecraft:coast_armor_trim_smithing_template_smithing_trim".into(),
            kind: SMITHING.into(),
            inputs: vec![
                v(&["minecraft:coast_armor_trim_smithing_template"]),
                v(&["minecraft:iron_chestplate"]),
                v(&["minecraft:gold_ingot"]),
            ],
            output: Some((v(&["minecraft:iron_chestplate"]), 1)),
            output_render_only: true,
            ..Parts::default()
        };
        let d = display(&parts).unwrap().1;
        assert_eq!(d.items_of(Role::Output).count(), 0);
        assert_eq!(
            d.items_of(Role::RenderOnly).collect::<Vec<_>>(),
            ["minecraft:iron_chestplate"]
        );
    }
}
