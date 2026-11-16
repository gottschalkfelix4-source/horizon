//! Ender IO for Horizon: the Alloy Smelter, the machine every Ender IO base starts from. It
//! takes three inputs, energy and a capacitor, and makes Ender IO's alloys (Electrical Steel,
//! Energetic Alloy, Vibrant Alloy, Dark Steel...).
//!
//! Written from Ender IO's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). The recipe list, energy costs and speeds are from memory and
//! listed in README.md as facts to check against the NeoForge original.
//!
//! How it works: `enderio:alloy_smelter` has a block entity with five item slots (three inputs,
//! one output, one capacitor; a hopper's menu shows them) and an energy buffer the game keeps
//! under `energy` in its saved compound, filled through the `horizon:energy` capability from any
//! side. Each tick the machine looks for the alloy recipe its inputs make, spends energy on it at
//! the rate its capacitor allows, and when the recipe's energy is paid, takes the inputs and
//! puts the alloy in the output slot.
use std::sync::OnceLock;

use mod_api::{
    BlockEntityPart, BlockEntityTickEvent, BlockPart, Compound, Hooks, ItemKey, ItemStack,
    ModContainer, ModEnergyBuffer, Registrar, RegistrationContext, ServerHost, Tag,
};

/// The machine's slots.
pub const INPUTS: [usize; 3] = [0, 1, 2];
/// Where the alloy goes.
pub const OUTPUT: usize = 3;
/// Where the capacitor sits; a machine without one does nothing.
pub const CAPACITOR: usize = 4;
/// The energy buffer, in µI (Ender IO's unit, one per Forge Energy point).
pub const ENERGY_CAPACITY: i32 = 100_000;
/// What the machine takes in per insert.
pub const ENERGY_INPUT: i32 = 1_000;
/// µI per tick spent with a basic capacitor; better capacitors multiply it.
pub const BASE_USE_PER_TICK: u32 = 20;
/// How many of one item fit in the output slot.
const MAX_STACK: u32 = 64;

/// One alloy recipe: its inputs (item and how many, in any slot order), what it makes, and the
/// energy it costs.
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe<K> {
    pub inputs: Vec<(K, u32)>,
    pub output: (K, u32),
    pub energy: u32,
}

/// The alloy recipes, by item id. From memory: see README.md.
pub const RECIPES: &[(&[(&str, u32)], (&str, u32), u32)] = &[
    (
        &[("minecraft:copper_ingot", 1), ("enderio:silicon", 1)],
        ("enderio:copper_alloy_ingot", 1),
        2400,
    ),
    (
        &[
            ("minecraft:iron_ingot", 1),
            ("minecraft:coal", 1),
            ("enderio:silicon", 1),
        ],
        ("enderio:electrical_steel_ingot", 1),
        3200,
    ),
    (
        &[
            ("minecraft:gold_ingot", 1),
            ("minecraft:redstone", 1),
            ("minecraft:glowstone_dust", 1),
        ],
        ("enderio:energetic_alloy_ingot", 1),
        4800,
    ),
    (
        &[
            ("enderio:energetic_alloy_ingot", 1),
            ("minecraft:ender_pearl", 1),
        ],
        ("enderio:vibrant_alloy_ingot", 1),
        4800,
    ),
    (
        &[("minecraft:redstone", 1), ("enderio:silicon", 1)],
        ("enderio:redstone_alloy_ingot", 1),
        3200,
    ),
    (
        &[
            ("enderio:copper_alloy_ingot", 1),
            ("minecraft:iron_ingot", 1),
            ("minecraft:redstone", 1),
        ],
        ("enderio:conductive_alloy_ingot", 1),
        3200,
    ),
    (
        &[("minecraft:iron_ingot", 1), ("minecraft:ender_pearl", 1)],
        ("enderio:pulsating_alloy_ingot", 1),
        4800,
    ),
    (
        &[
            ("minecraft:iron_ingot", 1),
            ("minecraft:coal", 1),
            ("minecraft:obsidian", 1),
        ],
        ("enderio:dark_steel_ingot", 1),
        6400,
    ),
    (
        &[("minecraft:gold_ingot", 1), ("minecraft:soul_sand", 1)],
        ("enderio:soularium_ingot", 1),
        5600,
    ),
    (
        &[
            ("minecraft:end_stone", 1),
            ("enderio:dark_steel_ingot", 1),
            ("minecraft:obsidian", 1),
        ],
        ("enderio:end_steel_ingot", 1),
        8000,
    ),
];

/// The capacitors and the factor each gives the machine's speed. From memory: see README.md.
pub const CAPACITORS: &[(&str, u32)] = &[
    ("enderio:basic_capacitor", 1),
    ("enderio:double_layer_capacitor", 2),
    ("enderio:octadic_capacitor", 3),
];

/// The first recipe the three input slots make, with the slot each of its inputs comes from.
/// Each input takes its own slot, which must hold at least the count; leftover items in the
/// slots are allowed, empty slots are fine, and a slot holding something no input uses blocks
/// the recipe, as in the machine's own slots.
pub fn find_recipe<K: PartialEq + Copy>(
    recipes: &[Recipe<K>],
    slots: &[Option<(K, u32)>; 3],
) -> Option<(usize, [Option<usize>; 3])> {
    'recipes: for (index, recipe) in recipes.iter().enumerate() {
        let mut used = [false; 3];
        let mut from = [None; 3];
        for (n, &(item, count)) in recipe.inputs.iter().enumerate() {
            let slot = (0..3).find(|&s| {
                !used[s] && matches!(slots[s], Some((held, have)) if held == item && have >= count)
            });
            let Some(slot) = slot else {
                continue 'recipes;
            };
            used[slot] = true;
            from[n] = Some(slot);
        }
        let stray = (0..3).any(|s| slots[s].is_some() && !used[s]);
        if !stray {
            return Some((index, from));
        }
    }
    None
}

/// Whether `count` of `item` fit in the output slot holding `held`.
pub fn output_fits<K: PartialEq>(held: Option<(K, u32)>, item: K, count: u32) -> bool {
    match held {
        None => count <= MAX_STACK,
        Some((have, n)) => have == item && n + count <= MAX_STACK,
    }
}

/// One tick of progress: what the machine spends from `energy` with a capacitor of `factor`
/// towards a recipe costing `cost` that has `progress` paid. Returns the energy spent and
/// whether the recipe is now paid in full.
pub fn spend(energy: u32, factor: u32, progress: u32, cost: u32) -> (u32, bool) {
    let rate = BASE_USE_PER_TICK * factor;
    let spent = rate.min(energy).min(cost.saturating_sub(progress));
    (spent, progress + spent >= cost)
}

/// Recipes and capacitors with their item keys, resolved at registration.
struct Tables {
    recipes: Vec<Recipe<ItemKey>>,
    capacitors: Vec<(ItemKey, u32)>,
}

static TABLES: OnceLock<Tables> = OnceLock::new();

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(n)) => *n,
        _ => 0,
    }
}

fn held(slot: &Option<ItemStack>) -> Option<(ItemKey, u32)> {
    slot.as_ref().map(|s| (s.item(), s.count()))
}

/// The alloy smelter's block entity.
struct AlloySmelter;

#[mod_api::horizon_part]
impl BlockPart for AlloySmelter {}

#[mod_api::horizon_part]
impl BlockEntityPart for AlloySmelter {
    fn block_entity_tick(&self, _: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let Some(tables) = TABLES.get() else {
            return;
        };
        let entity = &mut event.entity;
        if entity.items.len() <= CAPACITOR {
            return;
        }
        let factor = held(&entity.items[CAPACITOR]).and_then(|(item, _)| {
            tables
                .capacitors
                .iter()
                .find_map(|&(k, f)| (k == item).then_some(f))
        });
        let slots = INPUTS.map(|s| held(&entity.items[s]));
        let found = find_recipe(&tables.recipes, &slots);
        let recipe_code = found.map_or(-1, |(i, _)| i as i32);
        let mut progress = int(&entity.nbt, "progress").max(0) as u32;
        if int(&entity.nbt, "recipe") != recipe_code {
            progress = 0;
        }
        let (Some(factor), Some((index, from))) = (factor, found) else {
            write_state(&mut entity.nbt, recipe_code, progress);
            return;
        };
        let recipe = &tables.recipes[index];
        let (out, out_count) = recipe.output;
        if !output_fits(held(&entity.items[OUTPUT]), out, out_count) {
            write_state(&mut entity.nbt, recipe_code, progress);
            return;
        }
        let energy = int(&entity.nbt, "energy").max(0) as u32;
        let (spent, done) = spend(energy, factor, progress, recipe.energy);
        entity
            .nbt
            .insert("energy".into(), Tag::Int((energy - spent) as i32));
        progress += spent;
        if done {
            for (n, &(_, count)) in recipe.inputs.iter().enumerate() {
                let Some(slot) = from[n] else { continue };
                entity.items[slot] = entity.items[slot].take().and_then(|s| s.shrink(count));
            }
            entity.items[OUTPUT] = match entity.items[OUTPUT].take() {
                Some(mut s) => s.grow(out_count).is_ok().then_some(s),
                None => ItemStack::new(out, out_count),
            };
            progress = 0;
        }
        write_state(&mut entity.nbt, recipe_code, progress);
    }
}

fn write_state(nbt: &mut Compound, recipe: i32, progress: u32) {
    nbt.insert("recipe".into(), Tag::Int(recipe));
    nbt.insert("progress".into(), Tag::Int(progress as i32));
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let recipes = RECIPES
            .iter()
            .map(|(inputs, (out, n), energy)| Recipe {
                inputs: inputs.iter().map(|(id, c)| (r.item_key(id), *c)).collect(),
                output: (r.item_key(out), *n),
                energy: *energy,
            })
            .collect();
        let capacitors = CAPACITORS
            .iter()
            .map(|(id, f)| (r.item_key(id), *f))
            .collect();
        let _ = TABLES.set(Tables {
            recipes,
            capacitors,
        });

        let block = r.block_key("enderio:alloy_smelter");
        let sides: Vec<usize> = INPUTS.iter().copied().chain([OUTPUT]).collect();
        r.block_container(
            &block,
            ModContainer {
                slots: 5,
                faces: std::array::from_fn(|_| sides.clone()),
                inputs: INPUTS.to_vec(),
                outputs: vec![OUTPUT],
                menu: Some("minecraft:hopper".into()),
            },
        );
        r.block_energy(
            &block,
            ModEnergyBuffer {
                capacity: ENERGY_CAPACITY,
                max_insert: ENERGY_INPUT,
                max_extract: 0,
                insert_faces: [true; 6],
                extract_faces: [false; 6],
            },
        );
        let kind = r
            .block_entity_type("alloy_smelter")
            .blocks(&["alloy_smelter"])
            .build();
        let _ = r.block_entity_part(&kind, AlloySmelter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Vec<Recipe<&'static str>> {
        RECIPES
            .iter()
            .map(|(inputs, out, energy)| Recipe {
                inputs: inputs.to_vec(),
                output: *out,
                energy: *energy,
            })
            .collect()
    }

    #[test]
    fn inputs_match_in_any_slot_order_and_a_stray_item_blocks() {
        let recipes = table();
        let slots = [
            Some(("minecraft:glowstone_dust", 4)),
            None,
            Some(("minecraft:gold_ingot", 2)),
        ];
        // Gold and glowstone alone are not Energetic Alloy: the redstone is missing.
        assert_eq!(find_recipe(&recipes, &slots), None);
        let slots = [
            Some(("minecraft:glowstone_dust", 4)),
            Some(("minecraft:redstone", 1)),
            Some(("minecraft:gold_ingot", 2)),
        ];
        let (index, from) = find_recipe(&recipes, &slots).unwrap();
        assert_eq!(recipes[index].output.0, "enderio:energetic_alloy_ingot");
        assert_eq!(from, [Some(2), Some(1), Some(0)]);
        // An ender pearl beside iron is Pulsating Alloy; add dirt and nothing matches.
        let pulsating = [
            Some(("minecraft:ender_pearl", 1)),
            Some(("minecraft:iron_ingot", 1)),
            None,
        ];
        let (index, _) = find_recipe(&recipes, &pulsating).unwrap();
        assert_eq!(recipes[index].output.0, "enderio:pulsating_alloy_ingot");
        let stray = [pulsating[0], pulsating[1], Some(("minecraft:dirt", 1))];
        assert_eq!(find_recipe(&recipes, &stray), None);
    }

    #[test]
    fn a_better_capacitor_finishes_a_recipe_in_fewer_ticks() {
        let ticks = |factor| {
            let (mut progress, mut energy, mut n) = (0, 100_000, 0);
            loop {
                n += 1;
                let (spent, done) = spend(energy, factor, progress, 3200);
                energy -= spent;
                progress += spent;
                if done {
                    return (n, 100_000 - energy);
                }
            }
        };
        // 3200 µI at 20, 40 and 60 µI/t; the cost is the same whatever the speed.
        assert_eq!(ticks(1), (160, 3200));
        assert_eq!(ticks(2), (80, 3200));
        assert_eq!(ticks(3), (54, 3200));
    }

    #[test]
    fn no_energy_no_progress_and_a_full_output_waits() {
        assert_eq!(spend(0, 3, 100, 3200), (0, false));
        assert_eq!(spend(15, 1, 0, 3200), (15, false));
        assert!(output_fits(Some(("a", 63)), "a", 1));
        assert!(!output_fits(Some(("a", 64)), "a", 1));
        assert!(!output_fits(Some(("b", 1)), "a", 1));
        assert!(output_fits(None::<(&str, u32)>, "a", 1));
    }
}
