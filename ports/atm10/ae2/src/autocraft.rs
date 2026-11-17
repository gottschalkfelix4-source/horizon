//! The host side of autocrafting: reading patterns out of the network's pattern providers, encoding a
//! pattern from a crafting recipe, and running a plan against the network's cells.
//!
//! The port runs a request at once (AE2 spreads it over crafting CPUs and assemblers' time): it needs
//! the plan to succeed from the network's stock, and one molecular assembler with a channel. A craft's
//! result that no cell has room for goes to the player's inventory.
use std::collections::BTreeMap;

use mod_api::{Host, LevelRef, ServerHost, Text};

use crate::cells::*;
use crate::crafting::*;
use crate::{cells_of, network_extract, network_insert};

pub const PROVIDER_SLOTS: u32 = 9;
pub const BLANK: &str = "ae2:blank_pattern";
pub const PATTERN: &str = "ae2:crafting_pattern";

/// The patterns in the network's pattern providers.
pub fn patterns_of(host: &dyn Host, level: LevelRef, providers: &[[i32; 3]]) -> Vec<Pattern> {
    let mut out = Vec::new();
    for &p in providers {
        let Some(be) = host.block_entity(level, p) else { continue };
        for slot in 0..PROVIDER_SLOTS {
            if let Ok(Some(stack)) = be.item_slot(host, slot) {
                if let Some(pattern) = Pattern::of_stack(&stack) {
                    out.push(pattern);
                }
            }
        }
    }
    out
}

/// What the network's cells hold.
pub fn stock_of(host: &dyn Host, level: LevelRef, drives: &[[i32; 3]]) -> BTreeMap<String, u64> {
    let mut stock = BTreeMap::new();
    for (_, _, _, stack) in cells_of(host, level, drives) {
        for (id, n) in Store::of_stack(&stack).items {
            *stock.entry(id).or_insert(0) += n;
        }
    }
    stock
}

/// Why `encode` found nothing.
#[derive(Debug, PartialEq, Eq)]
pub enum EncodeError {
    NoRecipe,
    /// The recipe has an ingredient that names no item (a tag nothing is in).
    EmptyIngredient,
}

/// The item an ingredient stands for: its first in display order, else the first by id.
fn pick(items: &std::collections::HashSet<String>, order: &[String]) -> Option<String> {
    order.first().cloned().or_else(|| items.iter().min().cloned())
}

fn recipe_result(json: &mod_api::serde_json::Value) -> Option<(String, u32)> {
    let r = json.get("result")?;
    match r {
        mod_api::serde_json::Value::String(id) => Some((id.clone(), 1)),
        _ => Some((r.get("id")?.as_str()?.to_string(), r.get("count").and_then(|c| c.as_u64()).unwrap_or(1) as u32)),
    }
}

/// The ingredients of a shaped or shapeless recipe file, one per used slot.
fn json_ingredients(host: &dyn Host, json: &mod_api::serde_json::Value) -> Vec<mod_api::Ingredient> {
    let parse = |v: &mod_api::serde_json::Value| host.recipe_ingredient(&v.to_string()).ok().flatten();
    let mut out = Vec::new();
    if let (Some(rows), Some(key)) = (json.get("pattern").and_then(|p| p.as_array()), json.get("key")) {
        for row in rows.iter().filter_map(|r| r.as_str()) {
            for c in row.chars().filter(|c| *c != ' ') {
                if let Some(i) = key.get(c.to_string()).and_then(|v| parse(v)) {
                    out.push(i);
                }
            }
        }
    } else if let Some(list) = json.get("ingredients").and_then(|i| i.as_array()) {
        out.extend(list.iter().filter_map(|v| parse(v)));
    }
    out
}

/// A pattern for the first crafting recipe (shaped, shapeless or any `crafting` kind) that makes `id`.
pub fn encode(host: &dyn Host, id: &str) -> Result<Pattern, EncodeError> {
    let mut err = EncodeError::NoRecipe;
    for kind in ["minecraft:crafting_shaped", "minecraft:crafting_shapeless", "minecraft:crafting"] {
        for recipe in host.recipes_of_type(kind).iter() {
            // the typed parts are empty for some recipes: the recipe's file says the same
            let (result_id, out_n) = match recipe.results.first() {
                Some(r) => (r.item().id(host), r.count()),
                None => match recipe_result(&recipe.json) {
                    Some(r) => r,
                    None => continue,
                },
            };
            if result_id != id {
                continue;
            }
            let mut ingredients = recipe.ingredients.clone();
            if ingredients.is_empty() {
                ingredients = json_ingredients(host, &recipe.json);
            }
            let mut inputs: BTreeMap<String, u32> = BTreeMap::new();
            let mut whole = true;
            for ingredient in &ingredients {
                if ingredient.items.is_empty() && ingredient.order.is_empty() {
                    // an empty cell of a shaped recipe
                    continue;
                }
                match pick(&ingredient.items, &ingredient.order) {
                    Some(item) => *inputs.entry(item).or_insert(0) += 1,
                    None => whole = false,
                }
            }
            if whole && !inputs.is_empty() {
                return Ok(Pattern { inputs: inputs.into_iter().collect(), out: id.to_string(), out_n });
            }
            err = EncodeError::EmptyIngredient;
        }
    }
    Err(err)
}

/// How many crafting recipes the game has loaded (for a message when none fits).
pub fn crafting_recipe_count(host: &dyn Host) -> usize {
    ["minecraft:crafting_shaped", "minecraft:crafting_shapeless", "minecraft:crafting"]
        .iter()
        .map(|k| host.recipes_of_type(k).len())
        .sum()
}

fn tell(host: &mut dyn ServerHost, player: u128, msg: &str) {
    if let Some(p) = host.player(player) {
        let _ = p.action_bar(host, Text::literal(msg));
    }
}

/// Runs `n` of `want` for `player` over the network: plans, then crafts step by step.
pub fn request(
    host: &mut dyn ServerHost,
    level: LevelRef,
    player: u128,
    drives: &[[i32; 3]],
    providers: &[[i32; 3]],
    assemblers: usize,
    want: &str,
    n: u64,
) {
    if assemblers == 0 {
        return tell(host, player, "Autocrafting needs a molecular assembler with a channel");
    }
    let patterns = patterns_of(&*host, level, providers);
    let stock = stock_of(&*host, level, drives);
    let steps = match plan(&patterns, &stock, want, n) {
        Ok(s) => s,
        Err(e) => return tell(host, player, &e),
    };
    for step in steps {
        let pattern = &patterns[step.pattern];
        for _ in 0..step.times {
            let mut taken: Vec<(&String, u64)> = Vec::new();
            let mut ok = true;
            for (id, per) in &pattern.inputs {
                let got = network_extract(host, level, drives, id, *per as u64);
                taken.push((id, got));
                if got < *per as u64 {
                    ok = false;
                    break;
                }
            }
            if !ok {
                // a craft that cannot start returns what it took
                for (id, got) in taken {
                    network_insert(host, level, drives, id, got);
                }
                return tell(host, player, "Crafting stopped: the network ran out of an ingredient");
            }
            let put = network_insert(host, level, drives, &pattern.out, pattern.out_n as u64);
            let left = pattern.out_n as u64 - put;
            if left > 0 {
                if let Some(stack) = host
                    .item_key(&mod_api::Identifier::parse(&pattern.out).unwrap_or_else(|_| mod_api::Identifier::parse("minecraft:air").unwrap()))
                    .and_then(|k| mod_api::ItemStack::new(k, left as u32))
                {
                    if let Some(p) = host.player(player) {
                        let _ = p.give_stack(host, stack);
                    }
                }
            }
        }
    }
    tell(host, player, &format!("Crafted {want}"));
}
