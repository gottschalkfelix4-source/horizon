//! Farmer's Delight's rules, free of the game: numbers, state machines and recipe files.
//!
//! The glue in `lib.rs` reads the world, calls these and writes back what they decided.
use mod_api::serde_json::Value;

/// Cooking pot slots: six ingredients, the meal it holds, the container to serve it into, and
/// the served meal.
pub const POT_INPUTS: std::ops::Range<usize> = 0..6;
pub const POT_MEAL: usize = 6;
pub const POT_CONTAINER: usize = 7;
pub const POT_OUTPUT: usize = 8;
pub const POT_SLOTS: usize = 9;

/// The stove's grill holds six items, one per spot.
pub const STOVE_SLOTS: usize = 6;

/// Rich soil's chance, on each random tick, to grow the plant on it one stage
/// (`richSoilBoostChance`, default 0.2).
pub const RICH_SOIL_BOOST_CHANCE: f32 = 0.2;

/// The last stage of organic compost before it turns into rich soil.
pub const COMPOST_MAX: u8 = 7;

/// Whether a block heats a cooking pot or skillet standing on it, by its id and, for blocks
/// that can be out, its `lit` state. The `farmersdelight:heat_sources` tag is asked first by
/// the glue; this is the mod's default list, used when the tag is absent.
pub fn is_heat_source(block: &str, lit: Option<bool>) -> bool {
    match block {
        "minecraft:fire" | "minecraft:soul_fire" | "minecraft:lava" | "minecraft:magma_block" => {
            true
        }
        "minecraft:campfire" | "minecraft:soul_campfire" | "farmersdelight:stove" => lit == Some(true),
        _ => false,
    }
}

/// Assign each ingredient a distinct slot that it accepts, every filled slot used exactly once
/// (a shapeless recipe, as the cooking pot matches). Returns the slot of each ingredient, in
/// ingredient order, or None when the slots don't make the recipe.
pub fn match_shapeless<S, I>(
    slots: &[Option<S>],
    ingredients: &[I],
    accepts: impl Fn(&I, &S) -> bool,
) -> Option<Vec<usize>> {
    let filled: Vec<usize> = (0..slots.len()).filter(|&i| slots[i].is_some()).collect();
    if filled.len() != ingredients.len() || ingredients.is_empty() {
        return None;
    }
    // edges[i] = the filled slots ingredient i accepts
    let edges: Vec<Vec<usize>> = ingredients
        .iter()
        .map(|ing| {
            filled
                .iter()
                .copied()
                .filter(|&s| slots[s].as_ref().is_some_and(|stack| accepts(ing, stack)))
                .collect()
        })
        .collect();
    let mut owner: Vec<Option<usize>> = vec![None; slots.len()];
    fn augment(
        i: usize,
        edges: &[Vec<usize>],
        owner: &mut [Option<usize>],
        seen: &mut [bool],
    ) -> bool {
        for &s in &edges[i] {
            if seen[s] {
                continue;
            }
            seen[s] = true;
            if owner[s].is_none_or(|j| augment(j, edges, owner, seen)) {
                owner[s] = Some(i);
                return true;
            }
        }
        false
    }
    for i in 0..ingredients.len() {
        let mut seen = vec![false; slots.len()];
        if !augment(i, &edges, &mut owner, &mut seen) {
            return None;
        }
    }
    let mut out = vec![0; ingredients.len()];
    for (slot, o) in owner.iter().enumerate() {
        if let Some(i) = o {
            out[*i] = slot;
        }
    }
    Some(out)
}

/// A cooking timer, saved in the block entity's compound: the pot's and each stove or skillet
/// spot's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timer {
    pub time: i32,
    pub total: i32,
}

impl Timer {
    /// One tick: `recipe_time` is the recipe the contents make (None for none), `heated` whether
    /// there is heat, `room` whether the result has somewhere to go. True when it finished.
    /// Without heat or a recipe the progress falls back two ticks a tick, as a furnace's does.
    pub fn tick(&mut self, heated: bool, recipe_time: Option<i32>, room: bool) -> bool {
        match recipe_time {
            Some(total) if heated && room => {
                self.total = total.max(1);
                self.time += 1;
                if self.time >= self.total {
                    self.time = 0;
                    return true;
                }
                false
            }
            _ => {
                self.time = (self.time - 2).max(0);
                false
            }
        }
    }
}

/// Whether one more meal of `add` fits beside `held` of the same meal, up to `max_stack`.
pub fn fits(held: u32, add: u32, max_stack: u32) -> bool {
    held + add <= max_stack
}

/// One random tick of organic compost at stage `stage`: it composts one stage with a chance
/// that grows with what is around it (each activator next to it: mushrooms, rich soil, compost
/// and the like; water beside it; the sky above it). Returns the new stage; at
/// [`COMPOST_MAX`] it becomes rich soil (the glue swaps the block).
pub fn compost_tick(stage: u8, activators: u32, wet: bool, sky: bool, roll: f32) -> u8 {
    let chance = compost_chance(activators, wet, sky);
    if roll < chance { (stage + 1).min(COMPOST_MAX) } else { stage }
}

/// The chance above: 0.1 alone, plus 0.02 per activator (at most five), 0.02 for water and 0.02
/// for the sky.
pub fn compost_chance(activators: u32, wet: bool, sky: bool) -> f32 {
    0.1 + 0.02 * activators.min(5) as f32 + if wet { 0.02 } else { 0.0 } + if sky { 0.02 } else { 0.0 }
}

/// A feast block's servings: taking one with the right container leaves one fewer; an empty
/// feast is cleared away instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeastUse {
    /// Give one serving; the block keeps this many.
    Serve(u8),
    /// The feast is eaten: break the block (its leftovers drop).
    Clear,
    /// Not the container the feast needs: nothing happens, a hint is shown.
    NeedsContainer,
}

pub fn feast_use(servings: u8, holds_container: bool) -> FeastUse {
    if servings == 0 {
        FeastUse::Clear
    } else if holds_container {
        FeastUse::Serve(servings - 1)
    } else {
        FeastUse::NeedsContainer
    }
}

/// A feast block: its id, how many servings it holds, the serving and the container it is
/// served into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Feast {
    pub block: &'static str,
    pub servings: u8,
    pub serving: &'static str,
    pub container: &'static str,
}

pub const FEASTS: [Feast; 4] = [
    Feast {
        block: "farmersdelight:roast_chicken_block",
        servings: 4,
        serving: "farmersdelight:roast_chicken",
        container: "minecraft:bowl",
    },
    Feast {
        block: "farmersdelight:stuffed_pumpkin_block",
        servings: 4,
        serving: "farmersdelight:stuffed_pumpkin",
        container: "minecraft:bowl",
    },
    Feast {
        block: "farmersdelight:honey_glazed_ham_block",
        servings: 4,
        serving: "farmersdelight:honey_glazed_ham",
        container: "minecraft:bowl",
    },
    Feast {
        block: "farmersdelight:shepherds_pie_block",
        servings: 4,
        serving: "farmersdelight:shepherds_pie",
        container: "minecraft:bowl",
    },
];

/// One stack a recipe file names: item id and count.
#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    pub id: String,
    pub count: u32,
    /// The chance it is given (cutting board results), 1 for always.
    pub chance: f32,
}

/// A stack in a recipe file, written either `{"id": .., "count": ..}` (1.21) or
/// `{"item": .., "count": ..}` (older), or as a bare id.
pub fn read_stack(v: &Value) -> Option<Output> {
    if let Some(id) = v.as_str() {
        return Some(Output { id: id.to_string(), count: 1, chance: 1.0 });
    }
    let id = v.get("id").or_else(|| v.get("item"))?;
    // `{"item": {"id": .., "count": ..}, "chance": ..}`, the cutting board's result
    if id.is_object() {
        let mut inner = read_stack(id)?;
        inner.chance = v.get("chance").and_then(Value::as_f64).map_or(1.0, |c| c as f32);
        return Some(inner);
    }
    Some(Output {
        id: id.as_str()?.to_string(),
        count: v.get("count").and_then(Value::as_u64).map_or(1, |c| c as u32),
        chance: v.get("chance").and_then(Value::as_f64).map_or(1.0, |c| c as f32),
    })
}

/// An ingredient as 26.3's recipe files write it: 1.21's `{"item": id}` becomes `"id"`, `{"tag":
/// t}` becomes `"#t"`, and a list of either a list of those. Anything else (a custom ingredient)
/// is passed on as it is.
pub fn normalize_ingredient(v: &Value) -> String {
    fn one(v: &Value) -> Value {
        if let Some(id) = v.get("item").and_then(Value::as_str) {
            return Value::String(id.into());
        }
        if let Some(tag) = v.get("tag").and_then(Value::as_str) {
            return Value::String(format!("#{tag}"));
        }
        match v {
            Value::Array(list) => Value::Array(list.iter().map(one).collect()),
            other => other.clone(),
        }
    }
    one(v).to_string()
}

/// A `farmersdelight:cooking` recipe, as its file says it.
#[derive(Clone, Debug, PartialEq)]
pub struct CookingRecipe {
    /// Each ingredient's JSON text, for the game's ingredient parser.
    pub ingredients: Vec<String>,
    pub result: Output,
    /// The container a serving goes into (a bowl), if the meal needs one.
    pub container: Option<String>,
    pub cooking_time: i32,
    pub experience: f32,
}

pub fn read_cooking(json: &Value) -> Option<CookingRecipe> {
    let ingredients = json
        .get("ingredients")?
        .as_array()?
        .iter()
        .map(normalize_ingredient)
        .collect::<Vec<_>>();
    let result = read_stack(json.get("result")?)?;
    let container = json.get("container").and_then(read_stack).map(|s| s.id);
    Some(CookingRecipe {
        ingredients,
        result,
        container,
        cooking_time: json.get("cookingtime").and_then(Value::as_i64).map_or(200, |t| t as i32),
        experience: json.get("experience").and_then(Value::as_f64).unwrap_or(0.0) as f32,
    })
}

/// A `farmersdelight:cutting` recipe.
#[derive(Clone, Debug, PartialEq)]
pub struct CuttingRecipe {
    pub ingredient: String,
    /// The tool's ingredient JSON text: a tool action (`axe_dig`) is turned into the tag of
    /// the tools that have it.
    pub tool: String,
    pub results: Vec<Output>,
}

/// The item tag standing for a NeoForge tool action, for a cutting recipe's
/// `farmersdelight:tool_action` tool.
pub fn tool_action_tag(action: &str) -> Option<&'static str> {
    Some(match action {
        "axe_dig" | "axe_strip" => "#minecraft:axes",
        "pickaxe_dig" => "#minecraft:pickaxes",
        "shovel_dig" | "shovel_flatten" => "#minecraft:shovels",
        "hoe_dig" | "hoe_till" => "#minecraft:hoes",
        "shears_dig" | "shears_carve" | "shears_harvest" => "minecraft:shears",
        "sword_dig" => "#minecraft:swords",
        _ => return None,
    })
}

pub fn read_cutting(json: &Value) -> Option<CuttingRecipe> {
    let ingredient = normalize_ingredient(json.get("ingredients")?.as_array()?.first()?);
    let tool = json.get("tool")?;
    let tool = match tool.get("type").and_then(Value::as_str) {
        Some("farmersdelight:tool_action") => {
            Value::String(tool_action_tag(tool.get("action")?.as_str()?)?.into()).to_string()
        }
        _ => normalize_ingredient(tool),
    };
    let results = json
        .get("result")?
        .as_array()?
        .iter()
        .filter_map(read_stack)
        .collect::<Vec<_>>();
    (!results.is_empty()).then_some(CuttingRecipe { ingredient, tool, results })
}

/// The results a cut gives, each rolled against its chance with one draw from `rolls`.
pub fn roll_results(results: &[Output], mut rolls: impl FnMut() -> f32) -> Vec<(String, u32)> {
    results
        .iter()
        .filter(|r| r.chance >= 1.0 || rolls() < r.chance)
        .map(|r| (r.id.clone(), r.count))
        .collect()
}

/// A block-state property's value in the game's state string `ns:block[key=value,...]`.
pub fn state_property<'a>(state: &'a str, key: &str) -> Option<&'a str> {
    let props = state.split_once('[')?.1.strip_suffix(']')?;
    props.split(',').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then_some(v)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::serde_json::json;

    #[test]
    fn shapeless_matching_finds_an_assignment_a_greedy_one_misses() {
        // slot 0 holds "a", slot 1 "b"; ingredient 0 takes a or b, ingredient 1 only a.
        let slots = [Some("a"), Some("b"), None];
        let ings: [&[&str]; 2] = [&["a", "b"], &["a"]];
        let m = match_shapeless(&slots, &ings, |i, s| i.contains(s)).unwrap();
        assert_eq!(m, vec![1, 0]);
        // an extra item in the pot spoils the recipe, as a missing one does
        let slots = [Some("a"), Some("b"), Some("c")];
        assert_eq!(match_shapeless(&slots, &ings, |i, s| i.contains(s)), None);
        let slots = [Some("a"), None, None];
        assert_eq!(match_shapeless(&slots, &ings, |i, s| i.contains(s)), None);
    }

    #[test]
    fn a_timer_finishes_after_the_recipe_time_and_falls_back_without_heat() {
        let mut t = Timer::default();
        let done = (0..200).filter(|_| t.tick(true, Some(200), true)).count();
        assert_eq!(done, 1);
        for _ in 0..50 {
            t.tick(true, Some(200), true);
        }
        t.tick(false, Some(200), true);
        assert_eq!(t.time, 48);
        t.tick(true, Some(200), false);
        assert_eq!(t.time, 46);
    }

    #[test]
    fn heat_sources_count_only_when_lit() {
        assert!(is_heat_source("minecraft:magma_block", None));
        assert!(is_heat_source("farmersdelight:stove", Some(true)));
        assert!(!is_heat_source("farmersdelight:stove", Some(false)));
        assert!(!is_heat_source("minecraft:campfire", Some(false)));
        assert!(!is_heat_source("minecraft:stone", None));
    }

    #[test]
    fn compost_needs_a_roll_under_its_chance_and_stops_at_the_last_stage() {
        assert!((compost_chance(2, true, false) - 0.16).abs() < 1e-6);
        assert!((compost_chance(9, true, true) - 0.24).abs() < 1e-6);
        assert_eq!(compost_tick(3, 0, false, false, 0.05), 4);
        assert_eq!(compost_tick(3, 0, false, false, 0.15), 3);
        assert_eq!(compost_tick(COMPOST_MAX, 5, true, true, 0.0), COMPOST_MAX);
    }

    #[test]
    fn a_feast_serves_into_its_container_until_it_is_empty() {
        assert_eq!(feast_use(4, true), FeastUse::Serve(3));
        assert_eq!(feast_use(1, false), FeastUse::NeedsContainer);
        assert_eq!(feast_use(0, true), FeastUse::Clear);
    }

    #[test]
    fn cooking_recipes_read_both_stack_spellings() {
        let r = read_cooking(&json!({
            "type": "farmersdelight:cooking",
            "ingredients": [{"item": "minecraft:beef"}, {"tag": "c:vegetables"}],
            "result": {"id": "farmersdelight:beef_stew", "count": 1},
            "container": {"item": "minecraft:bowl"},
            "cookingtime": 150, "experience": 1.0
        }))
        .unwrap();
        assert_eq!(r.ingredients, ["\"minecraft:beef\"", "\"#c:vegetables\""]);
        assert_eq!(r.result.id, "farmersdelight:beef_stew");
        assert_eq!(r.container.as_deref(), Some("minecraft:bowl"));
        assert_eq!(r.cooking_time, 150);
        let r = read_cooking(&json!({"ingredients": [{"item": "a:b"}], "result": "a:c"})).unwrap();
        assert_eq!((r.cooking_time, r.container), (200, None));
    }

    #[test]
    fn cutting_recipes_turn_tool_actions_into_tool_tags_and_keep_chances() {
        let r = read_cutting(&json!({
            "type": "farmersdelight:cutting",
            "ingredients": [{"item": "minecraft:oak_log"}],
            "tool": {"type": "farmersdelight:tool_action", "action": "axe_strip"},
            "result": [
                {"item": {"id": "minecraft:stripped_oak_log", "count": 1}},
                {"item": {"id": "farmersdelight:tree_bark", "count": 1}, "chance": 0.5}
            ]
        }))
        .unwrap();
        assert_eq!(r.tool, "\"#minecraft:axes\"");
        assert_eq!(r.results[1].chance, 0.5);
        let mut draws = [0.7f32, 0.2].into_iter();
        assert_eq!(roll_results(&r.results, || draws.next().unwrap()).len(), 1);
        let mut draws = [0.2f32].into_iter();
        assert_eq!(roll_results(&r.results, || draws.next().unwrap()).len(), 2);
    }

    #[test]
    fn state_properties_are_read_from_the_state_string() {
        let s = "farmersdelight:stove[facing=north,lit=true]";
        assert_eq!(state_property(s, "lit"), Some("true"));
        assert_eq!(state_property(s, "age"), None);
        assert_eq!(state_property("minecraft:stone", "lit"), None);
    }
}
