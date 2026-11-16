//! Just Enough Breeding for Horizon: a JEI category with one recipe per animal, showing the
//! animal, the items it is bred with, and what breeding gives (its spawn egg for the baby, or
//! the egg it lays).
//!
//! Written from the mod's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). README.md says what still has to be checked against it.
//!
//! The breeding items are vanilla's food tags (`minecraft:cow_food`...), read from the tags the
//! client has, so a data pack's change to them shows.
use std::sync::Arc;

use jei::{Category, Display, JeiPlugin, Registration, Role};
use mod_api::{Compound, Hooks, Host, Identifier, Registrar, RegistrationContext};

pub const CATEGORY: &str = "justenoughbreeding:breeding";

/// A breedable animal: its type, the tag of what it is bred with, what breeding gives, and how
/// big it is drawn (GUI pixels per block; big animals smaller).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Animal {
    pub kind: &'static str,
    pub food: &'static str,
    pub offspring: &'static str,
    pub scale: f32,
}

const fn animal(
    kind: &'static str,
    food: &'static str,
    offspring: &'static str,
    scale: f32,
) -> Animal {
    Animal {
        kind,
        food,
        offspring,
        scale,
    }
}

/// Vanilla's breedable animals. An animal whose tag the client doesn't have is left out.
pub const ANIMALS: &[Animal] = &[
    animal(
        "minecraft:cow",
        "minecraft:cow_food",
        "minecraft:cow_spawn_egg",
        16.0,
    ),
    animal(
        "minecraft:mooshroom",
        "minecraft:cow_food",
        "minecraft:mooshroom_spawn_egg",
        16.0,
    ),
    animal(
        "minecraft:sheep",
        "minecraft:sheep_food",
        "minecraft:sheep_spawn_egg",
        16.0,
    ),
    animal(
        "minecraft:pig",
        "minecraft:pig_food",
        "minecraft:pig_spawn_egg",
        18.0,
    ),
    animal(
        "minecraft:chicken",
        "minecraft:chicken_food",
        "minecraft:chicken_spawn_egg",
        22.0,
    ),
    animal(
        "minecraft:horse",
        "minecraft:horse_food",
        "minecraft:horse_spawn_egg",
        12.0,
    ),
    animal(
        "minecraft:donkey",
        "minecraft:horse_food",
        "minecraft:donkey_spawn_egg",
        13.0,
    ),
    animal(
        "minecraft:llama",
        "minecraft:llama_food",
        "minecraft:llama_spawn_egg",
        12.0,
    ),
    animal(
        "minecraft:cat",
        "minecraft:cat_food",
        "minecraft:cat_spawn_egg",
        22.0,
    ),
    animal(
        "minecraft:ocelot",
        "minecraft:ocelot_food",
        "minecraft:ocelot_spawn_egg",
        22.0,
    ),
    animal(
        "minecraft:wolf",
        "minecraft:wolf_food",
        "minecraft:wolf_spawn_egg",
        20.0,
    ),
    animal(
        "minecraft:fox",
        "minecraft:fox_food",
        "minecraft:fox_spawn_egg",
        20.0,
    ),
    animal(
        "minecraft:rabbit",
        "minecraft:rabbit_food",
        "minecraft:rabbit_spawn_egg",
        24.0,
    ),
    animal(
        "minecraft:turtle",
        "minecraft:turtle_food",
        "minecraft:turtle_egg",
        18.0,
    ),
    animal(
        "minecraft:panda",
        "minecraft:panda_food",
        "minecraft:panda_spawn_egg",
        13.0,
    ),
    animal(
        "minecraft:bee",
        "minecraft:bee_food",
        "minecraft:bee_spawn_egg",
        24.0,
    ),
    animal(
        "minecraft:goat",
        "minecraft:goat_food",
        "minecraft:goat_spawn_egg",
        15.0,
    ),
    animal(
        "minecraft:strider",
        "minecraft:strider_food",
        "minecraft:strider_spawn_egg",
        14.0,
    ),
    animal(
        "minecraft:hoglin",
        "minecraft:hoglin_food",
        "minecraft:hoglin_spawn_egg",
        12.0,
    ),
    animal(
        "minecraft:frog",
        "minecraft:frog_food",
        "minecraft:frogspawn",
        24.0,
    ),
    animal(
        "minecraft:axolotl",
        "minecraft:axolotl_food",
        "minecraft:axolotl_spawn_egg",
        20.0,
    ),
    animal(
        "minecraft:camel",
        "minecraft:camel_food",
        "minecraft:camel_spawn_egg",
        10.0,
    ),
    animal(
        "minecraft:sniffer",
        "minecraft:sniffer_food",
        "minecraft:sniffer_egg",
        10.0,
    ),
    animal(
        "minecraft:armadillo",
        "minecraft:armadillo_food",
        "minecraft:armadillo_spawn_egg",
        22.0,
    ),
];

/// The recipe's size and the food slots it has room for (two rows of four).
pub const WIDTH: u16 = 160;
pub const HEIGHT: u16 = 46;
const FOOD_SLOTS: usize = 8;
const FOOD_COLUMNS: usize = 4;

/// One animal's recipe: the animal in a box at the left, its foods in up to eight slots (when
/// there are more, the last slot cycles through the rest), an arrow, and what breeding gives.
pub fn display(a: &Animal, foods: &[String]) -> Option<Display> {
    if foods.is_empty() {
        return None;
    }
    let mut d =
        Display::new(None).entity(a.kind, Compound::new(), [0, 0, 36, HEIGHT as i16], a.scale);
    let slots = foods.len().min(FOOD_SLOTS);
    for i in 0..slots {
        let items = if i == slots - 1 {
            foods[i..].to_vec()
        } else {
            vec![foods[i].clone()]
        };
        let pos = [
            41 + (i % FOOD_COLUMNS) as i16 * 18,
            6 + (i / FOOD_COLUMNS) as i16 * 18,
        ];
        d = d.slot(Role::Input, pos, items);
    }
    Some(
        d.arrow([116, 15])
            .slot(Role::Output, [143, 15], vec![a.offspring.to_owned()]),
    )
}

/// The plugin JEI asks.
pub struct BreedingPlugin;

impl JeiPlugin for BreedingPlugin {
    fn uid(&self) -> &str {
        "justenoughbreeding:jei"
    }
    fn register_categories(&self, _: &dyn Host, r: &mut Registration) {
        r.add_category(Category::new(
            CATEGORY,
            "gui.justenoughbreeding.category.breeding",
            "minecraft:wheat",
            WIDTH,
            HEIGHT,
        ));
    }
    fn register_recipes(&self, host: &dyn Host, r: &mut Registration) {
        let tags = host.tags();
        for a in ANIMALS {
            let Ok(id) = Identifier::parse(a.food) else {
                continue;
            };
            let Some(members) = tags.key(&id).and_then(|k| tags.items(k)) else {
                continue;
            };
            let foods: Vec<String> = members.into_iter().map(|k| k.id(host)).collect();
            if let Some(d) = display(a, &foods) {
                r.add_recipe(CATEGORY, d);
            }
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, _: &mut dyn Registrar, _: &RegistrationContext) {
        jei::register_plugin(Arc::new(BreedingPlugin));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jei::index::{Mode, RecipeIndex};

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn cow() -> &'static Animal {
        ANIMALS.iter().find(|a| a.kind == "minecraft:cow").unwrap()
    }

    #[test]
    fn more_foods_than_slots_cycle_in_the_last_one() {
        let foods: Vec<String> = (0..10).map(|i| format!("x:food{i}")).collect();
        let d = display(cow(), &foods).unwrap();
        let inputs: Vec<_> = d.slots.iter().filter(|s| s.role == Role::Input).collect();
        assert_eq!(inputs.len(), 8);
        assert_eq!(inputs[7].items, ids(&["x:food7", "x:food8", "x:food9"]));
        // The second row starts under the first.
        assert_eq!(inputs[4].pos, [41, 24]);
        assert!(display(cow(), &[]).is_none());
    }

    #[test]
    fn wheat_breeds_cows_and_sheep_and_a_turtle_lays_eggs() {
        let mut r = Registration::default();
        r.add_category(Category::new(
            CATEGORY,
            "k",
            "minecraft:wheat",
            WIDTH,
            HEIGHT,
        ));
        let sheep = ANIMALS
            .iter()
            .find(|a| a.kind == "minecraft:sheep")
            .unwrap();
        let turtle = ANIMALS
            .iter()
            .find(|a| a.kind == "minecraft:turtle")
            .unwrap();
        r.add_recipe(
            CATEGORY,
            display(cow(), &ids(&["minecraft:wheat"])).unwrap(),
        );
        r.add_recipe(
            CATEGORY,
            display(sheep, &ids(&["minecraft:wheat"])).unwrap(),
        );
        r.add_recipe(
            CATEGORY,
            display(turtle, &ids(&["minecraft:seagrass"])).unwrap(),
        );
        let index = RecipeIndex::build(r);
        assert_eq!(
            index.find("minecraft:wheat", Mode::Uses),
            vec![(0, vec![0, 1])]
        );
        assert_eq!(
            index.find("minecraft:turtle_egg", Mode::Recipes),
            vec![(0, vec![2])]
        );
        assert_eq!(
            index.find("minecraft:sheep_spawn_egg", Mode::Recipes),
            vec![(0, vec![1])]
        );
    }
}
