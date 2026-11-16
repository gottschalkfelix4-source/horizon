//! Sushi Go Crafting for Horizon: a rolling mat that turns rice, nori and fillings into rolls.
//!
//! Written from the mod's public listing, never from its code (docs/modpacks.md, "Licences").
//! README.md says how much of the original is assumed.
//!
//! `sushigocrafting:rolling_mat` has five slots in vanilla's hopper menu: rice, nori, two filling
//! slots and the result. With rice, nori and a filling in, and room in the result, it rolls for
//! 40 ticks, consumes one of each and makes a `sushi_roll` whose `minecraft:food` component is the
//! roll's value (`rules::roll`).
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Compound, Hooks, Host, Identifier, ItemStack, LevelSide,
    ModContainer, Registrar, RegistrationContext, ServerHost, Tag,
};

pub mod rules;
use rules::*;

const MAT: &str = "sushigocrafting:rolling_mat";
const RICE: &str = "sushigocrafting:rice";
const NORI: &str = "sushigocrafting:nori";
const ROLL: &str = "sushigocrafting:sushi_roll";

struct Mat;

#[mod_api::horizon_part]
impl BlockPart for Mat {}

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

/// The finished roll with its food value.
fn make_roll(host: &dyn Host, food: Food) -> Option<ItemStack> {
    let mut stack = ItemStack::new(host.item_key(&Identifier::parse(ROLL).ok()?)?, 1)?;
    let mut value = Compound::new();
    value.insert("nutrition".into(), Tag::Int(food.nutrition as i32));
    value.insert("saturation".into(), Tag::Float(food.saturation.to_bits()));
    let mut patch = Compound::new();
    patch.insert("minecraft:food".into(), Tag::Compound(value));
    stack.apply_components(&patch).ok()?;
    Some(stack)
}

#[mod_api::horizon_part]
impl BlockEntityPart for Mat {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == MAT).then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == MAT {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let mut items = event.entity.items.clone();
        if items.len() < 5 {
            items.resize(5, None);
        }
        let id = |s: &Option<ItemStack>| s.as_ref().map(|s| s.item().id(&*host));
        let fillings: Vec<String> = [2, 3].iter().filter_map(|i| id(&items[*i])).collect();
        let refs: Vec<&str> = fillings.iter().map(String::as_str).collect();
        let result = (id(&items[0]).as_deref() == Some(RICE) && id(&items[1]).as_deref() == Some(NORI))
            .then(|| roll(&refs))
            .flatten()
            .and_then(|food| make_roll(&*host, food))
            .filter(|r| match &items[4] {
                None => true,
                Some(held) => held.is_stackable_with(r) && held.count() < host.max_stack_size(held.item()).unwrap_or(64),
            });
        let mut progress = int(&event.entity.nbt, "progress");
        let before = progress;
        match result {
            None => progress = 0,
            Some(r) => {
                progress += 1;
                if progress >= ROLL_TICKS {
                    progress = 0;
                    for slot in [0, 1, 2, 3] {
                        items[slot] = items[slot].take().and_then(|s| s.shrink(1));
                    }
                    match &mut items[4] {
                        Some(held) => {
                            let _ = held.grow(1);
                        }
                        none => *none = Some(r),
                    }
                }
            }
        }
        if progress == before && items == event.entity.items {
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        }
        event.entity.nbt.insert("progress".into(), Tag::Int(progress));
        event.entity.items = items;
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let key = r.block_key(MAT);
        r.block_container(
            &key,
            ModContainer {
                slots: 5,
                faces: std::array::from_fn(|_| (0..5).collect()),
                inputs: vec![0, 1, 2, 3],
                outputs: vec![4],
                menu: Some("minecraft:hopper".into()),
            },
        );
        r.block_part(key, Mat);
        let kind = r.block_entity_type("rolling_mat").blocks(&[MAT]).build();
        let _ = r.block_entity_part(&kind, Mat);
    }
}
