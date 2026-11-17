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

/// A pattern for the first shaped or shapeless crafting recipe that makes `id`.
pub fn encode(host: &dyn Host, id: &str) -> Option<Pattern> {
    for kind in ["minecraft:crafting_shaped", "minecraft:crafting_shapeless"] {
        for recipe in host.recipes_of_type(kind).iter() {
            let Some(result) = recipe.results.first() else { continue };
            if result.item().id(host) != id {
                continue;
            }
            let mut inputs: BTreeMap<String, u32> = BTreeMap::new();
            let mut whole = true;
            for ingredient in &recipe.ingredients {
                match ingredient.order.first() {
                    Some(item) => *inputs.entry(item.clone()).or_insert(0) += 1,
                    // an empty cell of a shaped recipe has no items at all
                    None if ingredient.items.is_empty() => {}
                    None => whole = false,
                }
            }
            if whole && !inputs.is_empty() {
                return Some(Pattern { inputs: inputs.into_iter().collect(), out: id.to_string(), out_n: result.count() });
            }
        }
    }
    None
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
