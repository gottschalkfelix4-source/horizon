//! The choices a grid offers and the network form both halves share.

use mod_api::PayloadCodec;

/// One recipe the grid makes: its id and what it gives.
#[derive(Clone, Debug, PartialEq, Eq, PayloadCodec)]
pub struct Choice {
    /// The recipe's id (`minecraft:oak_planks`).
    pub recipe: String,
    /// The result's item id.
    pub item: String,
    /// The result's count.
    pub count: i32,
}

/// What the server tells a player's client: the choices for the grid they have open, empty
/// when there is nothing to choose (no recipe, or only one result).
#[derive(Clone, Debug, PartialEq, Eq, Default, PayloadCodec)]
pub struct Options {
    pub choices: Vec<Choice>,
}

/// The choices worth offering from every recipe the grid matches, in recipe order: one per
/// distinct result (two recipes that give the same stack are one choice, the first kept).
/// Fewer than two distinct results is nothing to choose.
pub fn offer(matches: Vec<Choice>) -> Vec<Choice> {
    let mut out: Vec<Choice> = Vec::new();
    for choice in matches {
        if !out
            .iter()
            .any(|c| c.item == choice.item && c.count == choice.count)
        {
            out.push(choice);
        }
    }
    if out.len() < 2 { Vec::new() } else { out }
}

/// Which of `choices` the result should be, given the player's last pick: the picked recipe
/// while the grid still makes it, else `None` (the game's own first match stays).
pub fn resolve(choices: &[Choice], picked: Option<&str>) -> Option<usize> {
    let picked = picked?;
    choices.iter().position(|c| c.recipe == picked)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::{decode_payload, encode_payload};

    fn choice(recipe: &str, item: &str, count: i32) -> Choice {
        Choice {
            recipe: recipe.into(),
            item: item.into(),
            count,
        }
    }

    #[test]
    fn only_distinct_results_are_offered_and_one_is_no_choice() {
        let two_mods = vec![
            choice("a:copper_block", "a:copper_block", 1),
            choice("b:copper_block", "b:copper_block", 1),
            choice("c:copper_block", "a:copper_block", 1),
        ];
        let offered = offer(two_mods);
        assert_eq!(
            offered
                .iter()
                .map(|c| c.recipe.as_str())
                .collect::<Vec<_>>(),
            ["a:copper_block", "b:copper_block"]
        );
        assert!(offer(vec![choice("a:x", "a:x", 1), choice("b:x", "a:x", 1)]).is_empty());
    }

    #[test]
    fn a_pick_holds_only_while_the_grid_still_makes_it() {
        let offered = vec![choice("a:slab", "a:slab", 6), choice("b:slab", "b:slab", 6)];
        assert_eq!(resolve(&offered, Some("b:slab")), Some(1));
        assert_eq!(resolve(&offered, Some("c:stairs")), None);
        assert_eq!(resolve(&offered, None), None);
    }

    #[test]
    fn options_cross_the_network_unchanged() {
        let options = Options {
            choices: vec![choice("a:slab", "a:slab", 6), choice("b:slab", "b:slab", 3)],
        };
        let bytes = encode_payload(&options, 1 << 20).unwrap();
        assert_eq!(decode_payload::<Options>(&bytes).unwrap(), options);
    }
}
