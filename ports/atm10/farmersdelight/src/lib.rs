//! Farmer's Delight for Horizon: the cooking pot, stove, skillet, cutting board, rich soil,
//! organic compost and feasts.
//!
//! Written from Farmer's Delight's observed behaviour and public description, never from its
//! code (docs/modpacks.md, "Licences"). What the port does, and what a harness against the
//! NeoForge original still has to confirm, is in README.md.
//!
//! The rules (timers, recipe files, shapeless matching, compost and feast steps) are in
//! [`rules`]; this file reads the world, asks them, and writes back. Recipes are the mod's own
//! data-pack files (`farmersdelight:cooking`, `farmersdelight:cutting`), read through
//! `recipes_of_type` and matched here, and vanilla's `minecraft:campfire_cooking` for the stove
//! and skillet, matched by the engine.
pub mod rules;

use std::sync::{Arc, Mutex};

use mod_api::serde_json::Value;
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockKey,
    BlockPart, BlockState, BlockUpdateFlags, Compound, Flow, Hand, Hooks, Host, HostTransaction,
    Identifier, Ingredient, InteractionResult, ItemPlace, ItemResource, ItemStack, LevelRef,
    LevelWriteHost, LightLayer, ModContainer, RandomSource, RandomTickEvent, Recipe, RecipeInput,
    Registrar, RegistrationContext, ServerHost, Tag, UseItemOnEvent, UseWithoutItemEvent,
};

use rules::*;

// ---------------------------------------------------------------------------------------------
// Small reads shared by the parts

fn state_string(host: &dyn Host, state: BlockState) -> String {
    host.block_state_string(state).unwrap_or_default()
}

fn block_id(state: &str) -> &str {
    state.split('[').next().unwrap_or(state)
}

fn stack_of(host: &dyn Host, id: &str, count: u32) -> Option<ItemStack> {
    let key = host.item_key(&Identifier::parse(id).ok()?)?;
    ItemStack::new(key, count)
}

fn parse_ingredient(host: &dyn Host, json: &str) -> Option<Ingredient> {
    host.recipe_ingredient(json).ok().flatten()
}

fn is_item(host: &dyn Host, stack: &ItemStack, id: &str) -> bool {
    stack.item().id(host) == id
}

/// One of `stack`.
fn one(stack: &ItemStack) -> ItemStack {
    let mut s = stack.clone();
    let _ = s.set_count(1);
    s
}

fn nbt_string(nbt: &Compound, key: &str) -> Option<String> {
    match nbt.get(&key.into()) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    }
}

fn int_array(nbt: &Compound, key: &str, len: usize) -> Vec<i32> {
    let mut v = match nbt.get(&key.into()) {
        Some(Tag::IntArray(a)) => a.clone(),
        _ => Vec::new(),
    };
    v.resize(len, 0);
    v
}

fn level(host: &dyn Host) -> Option<LevelRef> {
    host.level(host.dimension())
}

/// Whether the block under `pos` heats it: in `farmersdelight:heat_sources` (the tag the pack
/// fills), or on the mod's default list; a block with a `lit` state only while lit.
fn heated(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> bool {
    let below = [pos[0], pos[1] - 1, pos[2]];
    let Some(state) = level.block_state(host, below) else {
        return false;
    };
    let text = state_string(host, state);
    let lit = state_property(&text, "lit").map(|v| v == "true");
    let tagged = level
        .block(host, below)
        .and_then(|key| host.block_in_tag(key, "farmersdelight:heat_sources"))
        .unwrap_or(false);
    if tagged {
        return lit != Some(false);
    }
    is_heat_source(block_id(&text), lit)
}

/// Take `count` of what the clicking player holds (nothing in creative), in `tx`.
fn take_held(
    host: &mut dyn ServerHost,
    player: u128,
    hand: Hand,
    stack: &ItemStack,
    count: u32,
    tx: &mod_api::Transaction,
) -> bool {
    ItemPlace::PlayerHand { player, hand }.extract(host, tx, &ItemResource::of(stack), count) == count
}

/// Give the clicking player `stack`: into the hand if it can, else the inventory, else dropped.
fn give_player(
    host: &mut dyn ServerHost,
    player: u128,
    hand: Hand,
    stack: &ItemStack,
    tx: &mod_api::Transaction,
) {
    let _ = ItemPlace::PlayerHand { player, hand }.insert(host, tx, &ItemResource::of(stack), stack.count());
}

fn pop_above(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], stack: ItemStack) {
    let _ = level.spawn_item(
        host,
        f64::from(pos[0]) + 0.5,
        f64::from(pos[1]) + 1.0,
        f64::from(pos[2]) + 0.5,
        stack,
        10,
    );
}

/// The campfire recipe a stack makes, with its time.
fn campfire_recipe(host: &dyn Host, level: LevelRef, stack: &ItemStack) -> Option<(ItemStack, i32)> {
    let kind = Identifier::parse("minecraft:campfire_cooking").ok()?;
    let single = one(stack);
    let m = level.recipe_for(host, &kind, &RecipeInput::Single(single)).ok().flatten()?;
    Some((m.output, m.processing_time.max(1) as i32))
}

// ---------------------------------------------------------------------------------------------
// Recipe caches: the mod's own recipe types, parsed once per data-pack load

struct Parsed<T> {
    source: Arc<[Recipe]>,
    recipes: Vec<T>,
}

struct PotRecipe {
    recipe: CookingRecipe,
    ingredients: Vec<Ingredient>,
}

struct BoardRecipe {
    recipe: CuttingRecipe,
    ingredient: Ingredient,
    tool: Ingredient,
}

static POT_RECIPES: Mutex<Option<Parsed<PotRecipe>>> = Mutex::new(None);
static BOARD_RECIPES: Mutex<Option<Parsed<BoardRecipe>>> = Mutex::new(None);

fn with_recipes<T, R>(
    host: &dyn Host,
    cache: &Mutex<Option<Parsed<T>>>,
    kind: &str,
    parse: impl Fn(&dyn Host, &Value) -> Option<T>,
    f: impl FnOnce(&[T]) -> R,
) -> R {
    let source = host.recipes_of_type(kind);
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    let stale = guard.as_ref().is_none_or(|p| !Arc::ptr_eq(&p.source, &source));
    if stale {
        let recipes = source.iter().filter_map(|r| parse(host, &r.json)).collect();
        *guard = Some(Parsed { source, recipes });
    }
    f(&guard.as_ref().map_or(&[][..], |p| &p.recipes[..]))
}

fn parse_pot(host: &dyn Host, json: &Value) -> Option<PotRecipe> {
    let recipe = read_cooking(json)?;
    let ingredients = recipe
        .ingredients
        .iter()
        .map(|i| parse_ingredient(host, i))
        .collect::<Option<Vec<_>>>()?;
    Some(PotRecipe { recipe, ingredients })
}

fn parse_board(host: &dyn Host, json: &Value) -> Option<BoardRecipe> {
    let recipe = read_cutting(json)?;
    let ingredient = parse_ingredient(host, &recipe.ingredient)?;
    let tool = parse_ingredient(host, &recipe.tool)?;
    Some(BoardRecipe { recipe, ingredient, tool })
}

struct Keys {
    pot: BlockKey,
    stove: BlockKey,
    skillet: BlockKey,
    board: BlockKey,
}

fn is_block(event_block: &str, id: &str) -> bool {
    event_block == id
}

// ---------------------------------------------------------------------------------------------
// Cooking pot

struct CookingPot;

#[mod_api::horizon_part]
impl BlockPart for CookingPot {
    /// Right-clicking a pot that holds a meal with the meal's container serves one into it.
    fn use_item_on(&self, host: &mut dyn ServerHost, event: &mut UseItemOnEvent<'_>) -> Option<InteractionResult> {
        let click = event.click;
        let held = click.stack?.clone();
        let level = level(host)?;
        let be = host.block_entity(level, click.pos)?;
        let meal = be.item_slot(host, POT_MEAL as u32).ok().flatten()?;
        let data = be.data(host).unwrap_or_default();
        let container = nbt_string(&data, "container")?;
        if !is_item(host, &held, &container) {
            return None;
        }
        let served = one(&meal);
        let (player, hand) = (click.player, click.hand);
        let done = host.transaction(|host, tx| {
            if !take_held(host, player, hand, &held, 1, tx) {
                return Err(());
            }
            be.set_item_slot(host, tx, POT_MEAL as u32, meal.shrink(1)).map_err(|_| ())?;
            give_player(host, player, hand, &served, tx);
            Ok(())
        });
        done.is_ok().then_some(InteractionResult::Success)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for CookingPot {
    fn block_entity(&self, _: mod_api::LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        is_block(event.block, "farmersdelight:cooking_pot").then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if is_block(event.block, "farmersdelight:cooking_pot") {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let Some(level) = level(host) else { return };
        let entity = &mut *event.entity;
        let (items, nbt) = (&mut entity.items, &mut entity.nbt);
        if items.len() < POT_SLOTS {
            items.resize(POT_SLOTS, None);
        }
        let mut timer = Timer {
            time: int_array(nbt, "cook", 2)[0],
            total: int_array(nbt, "cook", 2)[1],
        };
        let before = timer;
        let mut changed = false;
        let hot = heated(host, level, pos);

        // which recipe the six ingredient slots make
        let inputs: Vec<Option<ItemStack>> = items[POT_INPUTS].to_vec();
        let found = if inputs.iter().any(Option::is_some) {
            with_recipes(host, &POT_RECIPES, "farmersdelight:cooking", parse_pot, |recipes| {
                recipes.iter().find_map(|r| {
                    let slots = match_shapeless(&inputs, &r.ingredients, |ing, stack| ing.test(host, stack))?;
                    Some((r.recipe.clone(), slots))
                })
            })
        } else {
            None
        };
        let result = found.as_ref().and_then(|(r, _)| stack_of(host, &r.result.id, r.result.count));
        let room = match (&items[POT_MEAL], &result) {
            (_, None) => false,
            (None, Some(_)) => true,
            (Some(held), Some(out)) => {
                let max = host.max_stack_size(held.item()).unwrap_or(64);
                held.is_stackable_with(out) && fits(held.count(), out.count(), max)
            }
        };
        let recipe_time = found.as_ref().map(|(r, _)| r.cooking_time);
        if timer.tick(hot, recipe_time, room) {
            if let (Some((recipe, slots)), Some(out)) = (found, result) {
                for slot in slots {
                    let Some(stack) = items[slot].take() else { continue };
                    let remainder = host.item_crafting_remainder(&stack);
                    items[slot] = stack.shrink(1);
                    if let Some(rem) = remainder {
                        if items[slot].is_none() {
                            items[slot] = Some(rem);
                        } else {
                            pop_above(host, level, pos, rem);
                        }
                    }
                }
                items[POT_MEAL] = match items[POT_MEAL].take() {
                    None => Some(out),
                    Some(mut held) => {
                        let _ = held.grow(out.count());
                        Some(held)
                    }
                };
                match recipe.container {
                    Some(c) => nbt.insert("container".into(), Tag::String(c.as_str().into())),
                    None => nbt.remove(&"container".into()),
                };
                changed = true;
            }
        }

        // serve: a meal and its container make a served meal in the output slot
        if let Some(meal) = items[POT_MEAL].clone() {
            let container = nbt_string(nbt, "container");
            let has_container = match (&container, &items[POT_CONTAINER]) {
                (None, _) => true,
                (Some(id), Some(c)) => is_item(host, c, id),
                (Some(_), None) => false,
            };
            let one = one(&meal);
            let out_room = match &items[POT_OUTPUT] {
                None => true,
                Some(o) => {
                    o.is_stackable_with(&one)
                        && fits(o.count(), 1, host.max_stack_size(o.item()).unwrap_or(64))
                }
            };
            if has_container && out_room {
                items[POT_MEAL] = meal.shrink(1);
                if container.is_some() {
                    items[POT_CONTAINER] = items[POT_CONTAINER].take().and_then(|c| c.shrink(1));
                }
                items[POT_OUTPUT] = match items[POT_OUTPUT].take() {
                    None => Some(one),
                    Some(mut o) => {
                        let _ = o.grow(1);
                        Some(o)
                    }
                };
                changed = true;
            }
        }

        if timer == before && !changed {
            *event.reply = mod_api::BlockEntityTickReply::Unchanged;
            return;
        }
        nbt.insert("cook".into(), Tag::IntArray(vec![timer.time, timer.total]));
    }
}

// ---------------------------------------------------------------------------------------------
// Stove: a lit grill of six spots that cooks campfire recipes and heats what stands on it

struct Stove;

#[mod_api::horizon_part]
impl BlockPart for Stove {
    fn use_item_on(&self, host: &mut dyn ServerHost, event: &mut UseItemOnEvent<'_>) -> Option<InteractionResult> {
        let click = event.click;
        let held = click.stack?.clone();
        let level = level(host)?;
        let text = state_string(host, event.state);
        let lit = state_property(&text, "lit") == Some("true");
        let (player, hand) = (click.player, click.hand);
        // a shovel puts it out, flint and steel or a fire charge lights it
        let shovel = parse_ingredient(host, "\"#minecraft:shovels\"").is_some_and(|i| i.test(host, &held));
        if shovel && lit {
            host.set_block_property(click.pos, "lit", "false");
            return Some(InteractionResult::Success);
        }
        if !lit
            && (is_item(host, &held, "minecraft:flint_and_steel")
                || is_item(host, &held, "minecraft:fire_charge"))
        {
            host.set_block_property(click.pos, "lit", "true");
            return Some(InteractionResult::Success);
        }
        // anything a campfire cooks goes on a free spot of the grill, from the top
        if click.hit.face != [0, 1, 0] {
            return None;
        }
        campfire_recipe(host, level, &held)?;
        let be = host.block_entity(level, click.pos)?;
        let free = (0..STOVE_SLOTS as u32).find(|&i| be.item_slot(host, i).ok().flatten().is_none())?;
        let one = one(&held);
        let done = host.transaction(|host, tx| {
            if !take_held(host, player, hand, &held, 1, tx) {
                return Err(());
            }
            be.set_item_slot(host, tx, free, Some(one)).map_err(|_| ())?;
            Ok(())
        });
        done.is_ok().then_some(InteractionResult::Success)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for Stove {
    fn block_entity(&self, _: mod_api::LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        is_block(event.block, "farmersdelight:stove").then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if is_block(event.block, "farmersdelight:stove") {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let Some(level) = level(host) else { return };
        let lit = level.block_property(host, pos, "lit").as_deref() == Some("true");
        let entity = &mut *event.entity;
        let (items, nbt) = (&mut entity.items, &mut entity.nbt);
        if items.len() < STOVE_SLOTS {
            items.resize(STOVE_SLOTS, None);
        }
        if items.iter().all(Option::is_none) {
            *event.reply = mod_api::BlockEntityTickReply::Unchanged;
            return;
        }
        let mut times = int_array(nbt, "cooking_times", STOVE_SLOTS);
        let mut totals = int_array(nbt, "cooking_totals", STOVE_SLOTS);
        let mut changed = false;
        for slot in 0..STOVE_SLOTS {
            let Some(stack) = items[slot].clone() else { continue };
            let recipe = campfire_recipe(host, level, &stack);
            let mut t = Timer { time: times[slot], total: totals[slot] };
            let before = t;
            if t.tick(lit, recipe.as_ref().map(|r| r.1), true) {
                if let Some((out, _)) = recipe {
                    items[slot] = None;
                    pop_above(host, level, pos, out);
                }
            }
            changed |= t != before;
            times[slot] = t.time;
            totals[slot] = t.total;
        }
        if !changed {
            *event.reply = mod_api::BlockEntityTickReply::Unchanged;
            return;
        }
        nbt.insert("cooking_times".into(), Tag::IntArray(times));
        nbt.insert("cooking_totals".into(), Tag::IntArray(totals));
    }
}

// ---------------------------------------------------------------------------------------------
// Skillet (placed): cooks the stack in it one item at a time over a heat source

struct Skillet;

#[mod_api::horizon_part]
impl BlockPart for Skillet {
    fn use_item_on(&self, host: &mut dyn ServerHost, event: &mut UseItemOnEvent<'_>) -> Option<InteractionResult> {
        let click = event.click;
        let held = click.stack?.clone();
        let level = level(host)?;
        campfire_recipe(host, level, &held)?;
        let be = host.block_entity(level, click.pos)?;
        if be.item_slot(host, 0).ok().flatten().is_some() {
            return None;
        }
        let (player, hand) = (click.player, click.hand);
        let count = held.count();
        let done = host.transaction(|host, tx| {
            if !take_held(host, player, hand, &held, count, tx) {
                return Err(());
            }
            be.set_item_slot(host, tx, 0, Some(held.clone())).map_err(|_| ())?;
            Ok(())
        });
        done.is_ok().then_some(InteractionResult::Success)
    }

    /// An empty hand takes back what is in the skillet.
    fn use_without_item(&self, host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
        take_back(host, event.click.pos, event.click.player, event.click.hand)
    }
}

/// Hand slot 0 of the block entity at `pos` back to the player.
fn take_back(host: &mut dyn ServerHost, pos: [i32; 3], player: u128, hand: Hand) -> Option<InteractionResult> {
    let level = level(host)?;
    let be = host.block_entity(level, pos)?;
    let stack = be.item_slot(host, 0).ok().flatten()?;
    let done = host.transaction(|host, tx| {
        be.set_item_slot(host, tx, 0, None).map_err(|_| ())?;
        give_player(host, player, hand, &stack, tx);
        Ok::<_, ()>(())
    });
    done.is_ok().then_some(InteractionResult::Success)
}

#[mod_api::horizon_part]
impl BlockEntityPart for Skillet {
    fn block_entity(&self, _: mod_api::LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        is_block(event.block, "farmersdelight:skillet").then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if is_block(event.block, "farmersdelight:skillet") {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let Some(level) = level(host) else { return };
        let entity = &mut *event.entity;
        let (items, nbt) = (&mut entity.items, &mut entity.nbt);
        if items.is_empty() {
            items.resize(1, None);
        }
        let Some(stack) = items[0].clone() else {
            *event.reply = mod_api::BlockEntityTickReply::Unchanged;
            return;
        };
        let c = int_array(nbt, "cook", 2);
        let mut t = Timer { time: c[0], total: c[1] };
        let before = t;
        let recipe = campfire_recipe(host, level, &stack);
        let hot = heated(host, level, pos);
        if t.tick(hot, recipe.as_ref().map(|r| r.1), true) {
            if let Some((out, _)) = recipe {
                items[0] = stack.shrink(1);
                pop_above(host, level, pos, out);
            }
        }
        if t == before {
            *event.reply = mod_api::BlockEntityTickReply::Unchanged;
            return;
        }
        nbt.insert("cook".into(), Tag::IntArray(vec![t.time, t.total]));
    }
}

// ---------------------------------------------------------------------------------------------
// Cutting board: put an item on it, then use the right tool on it

struct CuttingBoard;

#[mod_api::horizon_part]
impl BlockPart for CuttingBoard {
    fn use_item_on(&self, host: &mut dyn ServerHost, event: &mut UseItemOnEvent<'_>) -> Option<InteractionResult> {
        let click = event.click;
        let held = click.stack?.clone();
        let level = level(host)?;
        let be = host.block_entity(level, click.pos)?;
        let (player, hand, pos) = (click.player, click.hand, click.pos);
        match be.item_slot(host, 0).ok().flatten() {
            None => {
                // anything goes on an empty board, one at a time
                let one = one(&held);
                let done = host.transaction(|host, tx| {
                    if !take_held(host, player, hand, &held, 1, tx) {
                        return Err(());
                    }
                    be.set_item_slot(host, tx, 0, Some(one)).map_err(|_| ())?;
                    Ok(())
                });
                done.is_ok().then_some(InteractionResult::Success)
            }
            Some(on_board) => {
                let results = with_recipes(host, &BOARD_RECIPES, "farmersdelight:cutting", parse_board, |recipes| {
                    recipes
                        .iter()
                        .find(|r| r.ingredient.test(host, &on_board) && r.tool.test(host, &held))
                        .map(|r| r.recipe.results.clone())
                })?;
                let rolled = level
                    .with_random(host, |rng| roll_results(&results, || rng.next_float()))
                    .unwrap_or_default();
                let done = host.transaction(|host, tx| {
                    be.set_item_slot(host, tx, 0, None).map_err(|_| ())?;
                    Ok::<_, ()>(())
                });
                if done.is_err() {
                    return None;
                }
                for (id, count) in rolled {
                    if let Some(stack) = stack_of(host, &id, count) {
                        pop_above(host, level, pos, stack);
                    }
                }
                // the tool wears by one use
                if let Some(living) = host.entity(click.player_entity_id as u32).and_then(|e| host.living(e)) {
                    let slot = match hand {
                        Hand::Main => mod_api::EquipmentSlot::MainHand,
                        Hand::Off => mod_api::EquipmentSlot::OffHand,
                    };
                    let _ = living.hurt_and_break(host, slot, 1);
                }
                Some(InteractionResult::Success)
            }
        }
    }

    fn use_without_item(&self, host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
        take_back(host, event.click.pos, event.click.player, event.click.hand)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for CuttingBoard {
    fn block_entity(&self, _: mod_api::LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        is_block(event.block, "farmersdelight:cutting_board").then(Compound::new)
    }
}

// ---------------------------------------------------------------------------------------------
// Rich soil, mushroom colonies and organic compost

fn random_float(level: &mut dyn LevelWriteHost) -> Option<f32> {
    level.with_random(|r| r.next_float())
}

/// Grow what stands on rich soil: crops one stage (their `age`), mushrooms into colonies.
struct RichSoil;

#[mod_api::horizon_part]
impl BlockPart for RichSoil {
    fn random_tick(&self, level: &mut dyn LevelWriteHost, event: &mut RandomTickEvent) -> Flow {
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        let above = [pos[0], pos[1] + 1, pos[2]];
        let Some(state) = level.block_state(above) else { return Flow::Continue };
        let Some(text) = level.state_string(state) else { return Flow::Continue };
        let colony = match block_id(&text) {
            "minecraft:brown_mushroom" => Some("farmersdelight:brown_mushroom_colony"),
            "minecraft:red_mushroom" => Some("farmersdelight:red_mushroom_colony"),
            _ => None,
        };
        if let Some(colony) = colony {
            if let Some(c) = level.block_state_id(colony) {
                level.set_block(above, c, BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS);
            }
            return Flow::Continue;
        }
        let Some(age) = state_property(&text, "age").and_then(|a| a.parse::<u32>().ok()) else {
            return Flow::Continue;
        };
        let Some(next) = level.state_with_property(state, "age", &(age + 1).to_string()) else {
            return Flow::Continue;
        };
        if random_float(level).is_some_and(|r| r < RICH_SOIL_BOOST_CHANCE) {
            level.set_block(above, next, BlockUpdateFlags::CLIENTS);
        }
        Flow::Continue
    }
}

/// A colony grows (its `age`, 0 to 3) only on rich soil.
struct Colony;

#[mod_api::horizon_part]
impl BlockPart for Colony {
    /// Cancels the mushroom's own spreading: a colony grows in place.
    fn random_tick(&self, level: &mut dyn LevelWriteHost, event: &mut RandomTickEvent) -> Flow {
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        let below = [pos[0], pos[1] - 1, pos[2]];
        let on_rich_soil = level
            .block_state(below)
            .and_then(|s| level.state_block(s).map(|b| b == "farmersdelight:rich_soil"))
            .unwrap_or(false);
        let Some(text) = level.state_string(event.state) else { return Flow::Cancel };
        let age = state_property(&text, "age").and_then(|a| a.parse::<u32>().ok()).unwrap_or(0);
        if on_rich_soil && age < 3 && random_float(level).is_some_and(|r| r < RICH_SOIL_BOOST_CHANCE) {
            if let Some(next) = level.state_with_property(event.state, "age", &(age + 1).to_string()) {
                level.set_block(pos, next, BlockUpdateFlags::CLIENTS);
            }
        }
        Flow::Cancel
    }
}

struct OrganicCompost;

const COMPOST_ACTIVATORS: [&str; 6] = [
    "minecraft:brown_mushroom",
    "minecraft:red_mushroom",
    "minecraft:mycelium",
    "farmersdelight:rich_soil",
    "farmersdelight:organic_compost",
    "farmersdelight:brown_mushroom_colony",
];

#[mod_api::horizon_part]
impl BlockPart for OrganicCompost {
    fn random_tick(&self, level: &mut dyn LevelWriteHost, event: &mut RandomTickEvent) -> Flow {
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        let Some(text) = level.state_string(event.state) else { return Flow::Continue };
        let stage = state_property(&text, "composting").and_then(|a| a.parse::<u8>().ok()).unwrap_or(0);
        let mut activators = 0;
        let mut wet = false;
        for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
            let at = [pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]];
            let Some(s) = level.block_state(at) else { continue };
            let Some(block) = level.state_block(s).map(str::to_owned) else { continue };
            if block == "minecraft:water" {
                wet = true;
            }
            if COMPOST_ACTIVATORS.contains(&block.as_str()) {
                activators += 1;
            }
        }
        let sky = level.brightness(LightLayer::Sky, [pos[0], pos[1] + 1, pos[2]]) >= 15;
        let Some(roll) = random_float(level) else { return Flow::Continue };
        let next = compost_tick(stage, activators, wet, sky, roll);
        if next == stage {
            return Flow::Continue;
        }
        if next >= COMPOST_MAX {
            if let Some(soil) = level.block_state_id("farmersdelight:rich_soil") {
                level.set_block(pos, soil, BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS);
            }
        } else if let Some(s) = level.state_with_property(event.state, "composting", &next.to_string()) {
            level.set_block(pos, s, BlockUpdateFlags::CLIENTS);
        }
        Flow::Continue
    }
}

// ---------------------------------------------------------------------------------------------
// Feasts

struct FeastPart(Feast);

impl FeastPart {
    fn use_feast(&self, host: &mut dyn ServerHost, state: BlockState, click: &mod_api::BlockClick<'_>) -> Option<InteractionResult> {
        let level = level(host)?;
        let text = state_string(host, state);
        let servings = state_property(&text, "servings")?.parse::<u8>().ok()?;
        let held = click.stack.cloned();
        let holds = held.as_ref().is_some_and(|h| is_item(host, h, self.0.container));
        match feast_use(servings, holds) {
            FeastUse::Clear => {
                let _ = level.destroy_block(host, click.pos, true, None, false);
                Some(InteractionResult::Success)
            }
            FeastUse::NeedsContainer => Some(InteractionResult::Consume),
            FeastUse::Serve(left) => {
                let held = held?;
                let serving = stack_of(host, self.0.serving, 1)?;
                let (player, hand) = (click.player, click.hand);
                let done = host.transaction(|host, tx| {
                    if !take_held(host, player, hand, &held, 1, tx) {
                        return Err(());
                    }
                    give_player(host, player, hand, &serving, tx);
                    Ok(())
                });
                if done.is_err() {
                    return None;
                }
                host.set_block_property(click.pos, "servings", &left.to_string());
                Some(InteractionResult::Success)
            }
        }
    }
}

#[mod_api::horizon_part]
impl BlockPart for FeastPart {
    fn use_item_on(&self, host: &mut dyn ServerHost, event: &mut UseItemOnEvent<'_>) -> Option<InteractionResult> {
        self.use_feast(host, event.state, event.click)
    }

    fn use_without_item(&self, host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
        self.use_feast(host, event.state, event.click)
    }
}

// ---------------------------------------------------------------------------------------------

#[derive(Default)]
struct Mod;

fn no_faces() -> [Vec<usize>; 6] {
    Default::default()
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let keys = Keys {
            pot: r.block_key("farmersdelight:cooking_pot"),
            stove: r.block_key("farmersdelight:stove"),
            skillet: r.block_key("farmersdelight:skillet"),
            board: r.block_key("farmersdelight:cutting_board"),
        };

        // the pot: ingredients from above and the sides, the container from the sides, served
        // meals out of the bottom
        let inputs: Vec<usize> = POT_INPUTS.collect();
        r.block_container(
            &keys.pot,
            ModContainer {
                slots: POT_SLOTS,
                faces: [
                    vec![POT_OUTPUT],
                    inputs.clone(),
                    vec![POT_CONTAINER],
                    vec![POT_CONTAINER],
                    vec![POT_CONTAINER],
                    vec![POT_CONTAINER],
                ],
                inputs: [inputs, vec![POT_CONTAINER]].concat(),
                outputs: vec![POT_OUTPUT],
                menu: Some("minecraft:generic_3x3".into()),
                slot_limit: 0,
            },
        );
        r.block_container(
            &keys.stove,
            ModContainer { slots: STOVE_SLOTS, faces: no_faces(), inputs: vec![], outputs: vec![], menu: None,
slot_limit: 0, },
        );
        for key in [&keys.skillet, &keys.board] {
            r.block_container(
                key,
                ModContainer { slots: 1, faces: no_faces(), inputs: vec![], outputs: vec![], menu: None,
slot_limit: 0, },
            );
        }
        let pot = r.block_entity_type("cooking_pot").blocks(&["farmersdelight:cooking_pot"]).build();
        let _ = r.block_entity_part(&pot, CookingPot);
        let stove = r.block_entity_type("stove").blocks(&["farmersdelight:stove"]).build();
        let _ = r.block_entity_part(&stove, Stove);
        let skillet = r.block_entity_type("skillet").blocks(&["farmersdelight:skillet"]).build();
        let _ = r.block_entity_part(&skillet, Skillet);
        let board = r.block_entity_type("cutting_board").blocks(&["farmersdelight:cutting_board"]).build();
        let _ = r.block_entity_part(&board, CuttingBoard);

        for soil in ["farmersdelight:rich_soil", "farmersdelight:rich_soil_farmland"] {
            let key = r.block_key(soil);
            r.block_part(key, RichSoil);
        }
        for colony in ["farmersdelight:brown_mushroom_colony", "farmersdelight:red_mushroom_colony"] {
            let key = r.block_key(colony);
            r.block_part(key, Colony);
        }
        let compost = r.block_key("farmersdelight:organic_compost");
        r.block_part(compost, OrganicCompost);
        for feast in FEASTS {
            let key = r.block_key(feast.block);
            r.block_part(key, FeastPart(feast));
        }
    }
}
