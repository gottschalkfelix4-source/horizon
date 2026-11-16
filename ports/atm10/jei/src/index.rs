//! The recipe lookup behind "R" (recipes: how to get an item) and "U" (uses: what it goes
//! into), built once from what the plugins registered.
use std::collections::HashMap;

use crate::plugin::{Category, Display, Registration, Role};

/// Which question the player asked about an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// "R": recipes that make it.
    Recipes,
    /// "U": recipes that use it, and every recipe of a category it is the catalyst of.
    Uses,
}

/// The categories and recipes, with each item's recipes and uses.
#[derive(Debug, Default)]
pub struct RecipeIndex {
    pub categories: Vec<Category>,
    /// Per category, its recipes.
    pub recipes: Vec<Vec<Display>>,
    /// Per category, the items that make its recipes.
    pub catalysts: Vec<Vec<String>>,
    made_by: HashMap<String, Vec<(usize, usize)>>,
    used_by: HashMap<String, Vec<(usize, usize)>>,
    catalyst_of: HashMap<String, Vec<usize>>,
}

/// The answer to one question: the categories that have recipes for it, each with its recipes'
/// indices, in category order.
pub type Found = Vec<(usize, Vec<usize>)>;

impl RecipeIndex {
    pub fn build(r: Registration) -> Self {
        let Registration {
            categories,
            recipes,
            catalysts,
            ..
        } = r;
        let by_uid: HashMap<&str, usize> = categories
            .iter()
            .enumerate()
            .map(|(i, c)| (c.uid.as_str(), i))
            .collect();
        let mut index = Self {
            recipes: vec![Vec::new(); categories.len()],
            catalysts: vec![Vec::new(); categories.len()],
            ..Self::default()
        };
        for (uid, display) in recipes {
            let Some(&cat) = by_uid.get(uid.as_str()) else {
                continue;
            };
            let at = (cat, index.recipes[cat].len());
            for slot in &display.slots {
                let map = match slot.role {
                    Role::Output => &mut index.made_by,
                    Role::Input | Role::Catalyst => &mut index.used_by,
                    Role::RenderOnly => continue,
                };
                for item in &slot.items {
                    let list = map.entry(item.clone()).or_default();
                    if list.last() != Some(&at) {
                        list.push(at);
                    }
                }
            }
            index.recipes[cat].push(display);
        }
        for (uid, item) in catalysts {
            let Some(&cat) = by_uid.get(uid.as_str()) else {
                continue;
            };
            if !index.catalysts[cat].contains(&item) {
                index.catalysts[cat].push(item.clone());
                index.catalyst_of.entry(item).or_default().push(cat);
            }
        }
        index.categories = categories;
        index
    }

    /// The recipes answering `mode` for `item`; empty when there are none (JEI then does
    /// nothing).
    pub fn find(&self, item: &str, mode: Mode) -> Found {
        let mut found: Found = Vec::new();
        let mut add = |cat: usize, recipe: usize| match found.iter_mut().find(|(c, _)| *c == cat) {
            Some((_, list)) => {
                if !list.contains(&recipe) {
                    list.push(recipe);
                }
            }
            None => found.push((cat, vec![recipe])),
        };
        match mode {
            Mode::Recipes => {
                for &(cat, recipe) in self.made_by.get(item).into_iter().flatten() {
                    add(cat, recipe);
                }
            }
            Mode::Uses => {
                // A catalyst's whole category comes first, as JEI lists it.
                for &cat in self.catalyst_of.get(item).into_iter().flatten() {
                    for recipe in 0..self.recipes[cat].len() {
                        add(cat, recipe);
                    }
                }
                for &(cat, recipe) in self.used_by.get(item).into_iter().flatten() {
                    add(cat, recipe);
                }
            }
        }
        found.sort_by_key(|(cat, _)| *cat);
        found
    }

    /// Every recipe of category `cat` (a click on a tab's catalyst, JEI's "show all").
    pub fn all_of(&self, cat: usize) -> Found {
        if self.recipes.get(cat).is_none_or(Vec::is_empty) {
            return Vec::new();
        }
        vec![(cat, (0..self.recipes[cat].len()).collect())]
    }

    pub fn recipe(&self, cat: usize, recipe: usize) -> Option<&Display> {
        self.recipes.get(cat)?.get(recipe)
    }

    /// How many recipes there are in all.
    pub fn len(&self) -> usize {
        self.recipes.iter().map(Vec::len).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::Category;

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn sample() -> RecipeIndex {
        let mut r = Registration::default();
        r.add_category(Category::new(
            "minecraft:crafting",
            "k",
            "minecraft:crafting_table",
            116,
            54,
        ));
        r.add_category(Category::new(
            "minecraft:smelting",
            "k",
            "minecraft:furnace",
            82,
            54,
        ));
        // Planks from any log, sticks from planks, charcoal from logs.
        r.add_recipe(
            "minecraft:crafting",
            Display::new(Some("minecraft:oak_planks".into()))
                .input([0, 0], ids(&["minecraft:oak_log", "minecraft:oak_wood"]))
                .output([94, 18], ids(&["minecraft:oak_planks"])),
        );
        r.add_recipe(
            "minecraft:crafting",
            Display::new(Some("minecraft:stick".into()))
                .input([0, 0], ids(&["minecraft:oak_planks"]))
                .input([0, 18], ids(&["minecraft:oak_planks"]))
                .output([94, 18], ids(&["minecraft:stick"])),
        );
        r.add_recipe(
            "minecraft:smelting",
            Display::new(Some("minecraft:charcoal".into()))
                .input([0, 0], ids(&["minecraft:oak_log"]))
                .output([60, 18], ids(&["minecraft:charcoal"])),
        );
        r.add_recipe(
            "nobody:unknown",
            Display::new(None).output([0, 0], ids(&["minecraft:dirt"])),
        );
        r.add_catalyst("minecraft:smelting", "minecraft:furnace");
        RecipeIndex::build(r)
    }

    #[test]
    fn r_finds_what_makes_an_item_and_u_what_it_goes_into() {
        let index = sample();
        assert_eq!(
            index.find("minecraft:stick", Mode::Recipes),
            vec![(0, vec![1])]
        );
        // Oak logs make planks (crafting) and charcoal (smelting).
        assert_eq!(
            index.find("minecraft:oak_log", Mode::Uses),
            vec![(0, vec![0]), (1, vec![0])]
        );
        // Two slots of planks in one recipe list it once.
        assert_eq!(
            index.find("minecraft:oak_planks", Mode::Uses),
            vec![(0, vec![1])]
        );
        assert!(index.find("minecraft:stick", Mode::Uses).is_empty());
    }

    #[test]
    fn a_catalysts_uses_are_its_whole_category() {
        let index = sample();
        assert_eq!(
            index.find("minecraft:furnace", Mode::Uses),
            vec![(1, vec![0])]
        );
        assert_eq!(index.catalysts[1], ids(&["minecraft:furnace"]));
    }

    #[test]
    fn a_recipe_of_an_unknown_category_is_dropped() {
        let index = sample();
        assert!(index.find("minecraft:dirt", Mode::Recipes).is_empty());
        assert_eq!(index.len(), 3);
    }
}
