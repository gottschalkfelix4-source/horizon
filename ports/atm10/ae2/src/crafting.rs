//! Autocrafting: patterns, and the plan that turns a request into crafting steps.
//!
//! A pattern is an item (`ae2:crafting_pattern`) whose `minecraft:custom_data` holds
//! `ae2_pattern: {out: <item id>, out_n: <count>, in: {<item id>: <count>, ..}}`. The plan works on a
//! simulated stock: it takes what the network holds, crafts the rest with the pattern that makes it
//! (and, for its inputs, the same again), and fails when something is missing or recipes loop.
use std::collections::BTreeMap;

use mod_api::{Compound, ItemStack, Tag};

/// Deepest chain of crafts a plan follows.
pub const MAX_DEPTH: usize = 16;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Pattern {
    pub inputs: Vec<(String, u32)>,
    pub out: String,
    pub out_n: u32,
}

impl Pattern {
    pub fn of_stack(stack: &ItemStack) -> Option<Pattern> {
        let Some(Tag::Compound(data)) = stack.components().and_then(|c| c.as_nbt().get(&"minecraft:custom_data".into())) else {
            return None;
        };
        let Some(Tag::Compound(p)) = data.get(&"ae2_pattern".into()) else { return None };
        let out = match p.get(&"out".into()) {
            Some(Tag::String(s)) => s.to_string_lossy(),
            _ => return None,
        };
        let out_n = match p.get(&"out_n".into()) {
            Some(Tag::Int(n)) if *n > 0 => *n as u32,
            _ => return None,
        };
        let Some(Tag::Compound(inp)) = p.get(&"in".into()) else { return None };
        let mut inputs: Vec<(String, u32)> = inp
            .iter()
            .filter_map(|(k, v)| match v {
                Tag::Int(n) if *n > 0 => Some((k.to_string_lossy(), *n as u32)),
                _ => None,
            })
            .collect();
        inputs.sort();
        (!inputs.is_empty()).then_some(Pattern { inputs, out, out_n })
    }

    pub fn write_to(&self, stack: &mut ItemStack) {
        let mut inp = Compound::new();
        for (id, n) in &self.inputs {
            inp.insert(id.as_str().into(), Tag::Int(*n as i32));
        }
        let mut p = Compound::new();
        p.insert("out".into(), Tag::String(self.out.as_str().into()));
        p.insert("out_n".into(), Tag::Int(self.out_n as i32));
        p.insert("in".into(), Tag::Compound(inp));
        let mut data = Compound::new();
        data.insert("ae2_pattern".into(), Tag::Compound(p));
        let mut patch = Compound::new();
        patch.insert("minecraft:custom_data".into(), Tag::Compound(data));
        let _ = stack.apply_components(&patch);
    }
}

/// One crafting step: pattern number `pattern`, run `times` times.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Step {
    pub pattern: usize,
    pub times: u64,
}

fn need(
    patterns: &[Pattern],
    stock: &mut BTreeMap<String, u64>,
    id: &str,
    n: u64,
    depth: usize,
    steps: &mut Vec<Step>,
) -> Result<(), String> {
    let have = stock.get(id).copied().unwrap_or(0);
    let take = have.min(n);
    if take > 0 {
        stock.insert(id.to_string(), have - take);
    }
    let missing = n - take;
    if missing == 0 {
        return Ok(());
    }
    if depth >= MAX_DEPTH {
        return Err(format!("crafting chain for {id} is too deep or loops"));
    }
    let Some(index) = patterns.iter().position(|p| p.out == id) else {
        return Err(format!("missing {missing} of {id}"));
    };
    let times = missing.div_ceil(patterns[index].out_n as u64);
    for (input, per) in &patterns[index].inputs {
        need(patterns, stock, input, *per as u64 * times, depth + 1, steps)?;
    }
    // what the crafts make beyond the need stays in stock
    let surplus = times * patterns[index].out_n as u64 - missing;
    if surplus > 0 {
        *stock.entry(id.to_string()).or_insert(0) += surplus;
    }
    steps.push(Step { pattern: index, times });
    Ok(())
}

/// The steps (inputs' crafts first) that make `n` of `want` from `stock`, or why they cannot.
pub fn plan(patterns: &[Pattern], stock: &BTreeMap<String, u64>, want: &str, n: u64) -> Result<Vec<Step>, String> {
    let mut stock = stock.clone();
    let mut steps = Vec::new();
    // asking for something the network holds is no craft: the plan crafts it regardless
    let Some(index) = patterns.iter().position(|p| p.out == want) else {
        return Err(format!("no pattern makes {want}"));
    };
    let times = n.div_ceil(patterns[index].out_n as u64);
    for (input, per) in &patterns[index].inputs {
        need(patterns, &mut stock, input, *per as u64 * times, 1, &mut steps)?;
    }
    steps.push(Step { pattern: index, times });
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pat(inputs: &[(&str, u32)], out: &str, n: u32) -> Pattern {
        Pattern { inputs: inputs.iter().map(|(i, c)| (i.to_string(), *c)).collect(), out: out.into(), out_n: n }
    }

    fn stock(items: &[(&str, u64)]) -> BTreeMap<String, u64> {
        items.iter().map(|(i, c)| (i.to_string(), *c)).collect()
    }

    #[test]
    fn a_plan_crafts_the_missing_inputs_first() {
        // planks <- 1 log (4 planks); stick <- 2 planks (4 sticks); a pickaxe wants 3 stone + 2 sticks
        let pats = [
            pat(&[("log", 1)], "planks", 4),
            pat(&[("planks", 2)], "stick", 4),
            pat(&[("stone", 3), ("stick", 2)], "pickaxe", 1),
        ];
        let steps = plan(&pats, &stock(&[("log", 5), ("stone", 3)]), "pickaxe", 1).unwrap();
        assert_eq!(
            steps,
            [
                Step { pattern: 0, times: 1 },
                Step { pattern: 1, times: 1 },
                Step { pattern: 2, times: 1 }
            ]
        );
    }

    #[test]
    fn what_the_network_holds_is_not_crafted() {
        let pats = [pat(&[("planks", 2)], "stick", 4), pat(&[("log", 1)], "planks", 4)];
        let steps = plan(&pats, &stock(&[("planks", 10)]), "stick", 8).unwrap();
        assert_eq!(steps, [Step { pattern: 0, times: 2 }]);
    }

    #[test]
    fn a_missing_raw_material_is_named() {
        let pats = [pat(&[("stone", 3), ("stick", 2)], "pickaxe", 1)];
        let err = plan(&pats, &stock(&[("stone", 3)]), "pickaxe", 1).unwrap_err();
        assert_eq!(err, "missing 2 of stick");
    }

    #[test]
    fn surplus_of_one_craft_serves_the_next_need() {
        // b needs 1 plank and 1 a, a needs 1 plank: one craft of 4 planks covers both
        let pats = [
            pat(&[("log", 1)], "planks", 4),
            pat(&[("planks", 1)], "a", 1),
            pat(&[("planks", 1), ("a", 1)], "b", 1),
        ];
        let steps = plan(&pats, &stock(&[("log", 1)]), "b", 1).unwrap();
        assert_eq!(
            steps,
            [
                Step { pattern: 0, times: 1 },
                Step { pattern: 1, times: 1 },
                Step { pattern: 2, times: 1 }
            ]
        );
    }

    #[test]
    fn loops_stop() {
        let pats = [pat(&[("b", 1)], "a", 1), pat(&[("a", 1)], "b", 1)];
        assert!(plan(&pats, &stock(&[]), "a", 1).unwrap_err().contains("loops"));
    }

    #[test]
    fn no_pattern_no_plan() {
        assert!(plan(&[], &stock(&[("x", 5)]), "x", 1).is_err());
    }
}
