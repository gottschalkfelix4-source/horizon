//! Occultism for Horizon: Spiritfire, the otherworldly fire that turns items thrown into it into
//! occult materials instead of burning them.
//!
//! Written from Occultism's public description (its Dictionary of Spirits guide book, the wiki,
//! how the mod plays), never from its code. What still has to be confirmed against the NeoForge
//! original is listed in README.md.
//!
//! How it works:
//! - A Demon's Dream fruit (`occultism:datura`) dropped into an ordinary fire (or soul fire)
//!   is used up and the fire becomes `occultism:spirit_fire`.
//! - An item entity of a spirit fire recipe's input lying in spirit fire turns into the recipe's
//!   result, the whole stack at once, keeping its count.
//!
//! Both happen in `on_entity_item_update`, the item entity's server tick, on parts attached to
//! the fruit and to every recipe input: an item with no part costs nothing.
use std::sync::OnceLock;

use mod_api::{
    BlockKey, BlockUpdateFlags, Flow, ItemEntityRef, ItemKey, ItemPart, ItemStack,
    OnEntityItemUpdateEvent, Registrar, RegistrationContext, ServerHost,
};

/// One spirit fire transformation (`occultism:spirit_fire` recipes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpiritFireRecipe {
    pub input: &'static str,
    pub output: &'static str,
}

/// The transformations this port knows (from memory: the original reads them from its data
/// pack's `occultism:spirit_fire` recipes, which must replace this table).
pub const RECIPES: [SpiritFireRecipe; 2] = [
    SpiritFireRecipe {
        input: "minecraft:diamond",
        output: "occultism:spirit_attuned_gem",
    },
    SpiritFireRecipe {
        input: "minecraft:andesite",
        output: "occultism:otherstone",
    },
];

/// What lies under an item entity, as far as spirit fire cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Under {
    /// `minecraft:fire` or `minecraft:soul_fire`.
    OrdinaryFire,
    SpiritFire,
    Other,
}

/// What an item does this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Nothing,
    /// Use up one fruit and turn the fire into spirit fire.
    Kindle,
    /// Replace the stack with `count` of `RECIPES[recipe].output`.
    Transform {
        recipe: usize,
        count: u32,
    },
}

/// What kind of item a part was attached to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Thrown {
    Fruit,
    Input(usize),
}

/// Decide what a thrown item does over `under`.
pub fn action(thrown: Thrown, count: u32, under: Under) -> Action {
    match (thrown, under) {
        (Thrown::Fruit, Under::OrdinaryFire) if count > 0 => Action::Kindle,
        (Thrown::Input(recipe), Under::SpiritFire) if count > 0 => {
            Action::Transform { recipe, count }
        }
        _ => Action::Nothing,
    }
}

struct Keys {
    spirit_fire: BlockKey,
    fires: [BlockKey; 2],
    fruit: ItemKey,
    inputs: Vec<ItemKey>,
    outputs: Vec<ItemKey>,
}

static KEYS: OnceLock<Keys> = OnceLock::new();

/// Attached to the fruit and to every recipe input.
struct Thrownable;

impl Thrownable {
    fn run(host: &mut dyn ServerHost, keys: &Keys, stack: &ItemStack, item: ItemEntityRef) {
        let thrown = if stack.item() == keys.fruit {
            Thrown::Fruit
        } else if let Some(i) = keys.inputs.iter().position(|k| *k == stack.item()) {
            Thrown::Input(i)
        } else {
            return;
        };
        let entity = item.entity();
        let (Some(level), Some(pos)) = (entity.level(host), entity.block_position(host)) else {
            return;
        };
        let under = match level.block(host, pos) {
            Some(b) if b == keys.spirit_fire => Under::SpiritFire,
            Some(b) if keys.fires.contains(&b) => Under::OrdinaryFire,
            _ => Under::Other,
        };
        match action(thrown, stack.count(), under) {
            Action::Nothing => {}
            Action::Kindle => {
                let Some(state) = host.block_state_default(keys.spirit_fire) else {
                    return;
                };
                let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
                if level.set_block(host, pos, state, flags) != Ok(true) {
                    return;
                }
                match stack.clone().shrink(1) {
                    Some(rest) => {
                        let _ = item.set_stack(host, rest);
                    }
                    None => {
                        entity.discard(host);
                    }
                }
            }
            Action::Transform { recipe, count } => {
                let result = match data_result(host, stack) {
                    Some(made) => ItemStack::new(made.item(), made.count() * count),
                    None => ItemStack::new(keys.outputs[recipe], count),
                };
                let Some(result) = result else {
                    return;
                };
                if item.set_stack(host, result) == Ok(true) {
                    if let Some(at) = entity.position(host) {
                        level.play_sound(
                            host,
                            "minecraft:block.fire.extinguish",
                            at,
                            mod_api::SoundSource::Blocks,
                            0.5,
                            1.2,
                        );
                    }
                }
            }
        }
    }
}

/// The result the loaded `occultism:spirit_fire` recipes give for one of `stack`, when a data
/// pack (the jar's, carried) has them: `{"ingredient": <ingredient>, "result": <stack>}`.
/// `None` with no such recipes or none matching, and the fixed table answers.
fn data_result(host: &dyn ServerHost, stack: &ItemStack) -> Option<ItemStack> {
    host.recipes_of_type(RECIPE_TYPE).iter().find_map(|recipe| {
        let ingredient = recipe.json.get("ingredient")?.to_string();
        let ingredient = host.recipe_ingredient(&ingredient).ok()??;
        if !ingredient.test(host, stack) {
            return None;
        }
        let result = recipe.json.get("result")?.to_string();
        host.recipe_result(&result).ok()?
    })
}

/// The original's recipe type for spirit fire.
pub const RECIPE_TYPE: &str = "occultism:spirit_fire";

#[mod_api::horizon_part]
impl ItemPart for Thrownable {
    fn on_entity_item_update(
        &self,
        host: &mut dyn ServerHost,
        event: &mut OnEntityItemUpdateEvent<'_>,
    ) -> Flow {
        if let Some(keys) = KEYS.get() {
            Self::run(host, keys, event.stack, event.item);
        }
        Flow::Continue
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl mod_api::Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let keys = Keys {
            spirit_fire: r.block_key("occultism:spirit_fire"),
            fires: [
                r.block_key("minecraft:fire"),
                r.block_key("minecraft:soul_fire"),
            ],
            fruit: r.item_key("occultism:datura"),
            inputs: RECIPES.iter().map(|x| r.item_key(x.input)).collect(),
            outputs: RECIPES.iter().map(|x| r.item_key(x.output)).collect(),
        };
        r.item_part(keys.fruit, Thrownable);
        for input in &keys.inputs {
            r.item_part(*input, Thrownable);
        }
        let _ = KEYS.set(keys);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fruit_kindles_only_ordinary_fire() {
        assert_eq!(
            action(Thrown::Fruit, 3, Under::OrdinaryFire),
            Action::Kindle
        );
        assert_eq!(action(Thrown::Fruit, 3, Under::SpiritFire), Action::Nothing);
        assert_eq!(action(Thrown::Fruit, 3, Under::Other), Action::Nothing);
    }

    #[test]
    fn spirit_fire_turns_the_whole_stack_and_keeps_its_count() {
        assert_eq!(
            action(Thrown::Input(0), 7, Under::SpiritFire),
            Action::Transform {
                recipe: 0,
                count: 7
            }
        );
    }

    #[test]
    fn ordinary_fire_does_not_transform() {
        assert_eq!(
            action(Thrown::Input(1), 4, Under::OrdinaryFire),
            Action::Nothing
        );
        assert_eq!(action(Thrown::Input(1), 4, Under::Other), Action::Nothing);
    }
}
