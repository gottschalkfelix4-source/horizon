//! BotanyPots for Horizon: pots that grow a crop from a soil and a seed.
//!
//! Written from BotanyPots' observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"); the original is LGPL-2.1 and so is this port. README.md lists
//! what is ported and what a check against the NeoForge original still has to confirm.
//!
//! `botanypots:botany_pot` has nine slots in vanilla's 9x1 menu: soil, seed and seven output slots.
//! Hoppers put soil and seed in and take the harvest out. While the pair matches
//! (`rules::growth_ticks`) the pot counts ticks; when ripe it adds the harvest to the output slots
//! if all of it fits, else waits.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Compound, Hooks, Host, Identifier, ItemStack, LevelSide,
    ModContainer, Registrar, RegistrationContext, ServerHost, Tag,
};

pub mod rules;
use rules::*;

const POT: &str = "botanypots:botany_pot";
const SOIL: usize = 0;
const SEED: usize = 1;
const OUTPUTS: std::ops::Range<usize> = 2..9;

/// Adds `add` to the output slots of `items`: merging into matching stacks, then empty slots.
fn insert(host: &dyn Host, items: &mut [Option<ItemStack>], mut add: ItemStack) -> bool {
    let max = host.max_stack_size(add.item()).unwrap_or(64);
    for slot in OUTPUTS {
        let Some(held) = items[slot].as_mut() else { continue };
        if held.is_stackable_with(&add) && held.count() < max {
            let room = max - held.count();
            let moved = room.min(add.count());
            let _ = held.grow(moved);
            match add.clone().shrink(moved) {
                Some(rest) => add = rest,
                None => return true,
            }
        }
    }
    for slot in OUTPUTS {
        if items[slot].is_none() {
            let moved = max.min(add.count());
            items[slot] = ItemStack::new(add.item(), moved);
            match add.clone().shrink(moved) {
                Some(rest) => add = rest,
                None => return true,
            }
        }
    }
    false
}

struct Pot;

#[mod_api::horizon_part]
impl BlockPart for Pot {}

#[mod_api::horizon_part]
impl BlockEntityPart for Pot {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == POT).then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == POT {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let mut items = event.entity.items.clone();
        if items.len() < 9 {
            items.resize(9, None);
        }
        let soil = items[SOIL].as_ref().map(|s| s.item().id(&*host));
        let seed = items[SEED].as_ref().map(|s| s.item().id(&*host));
        let progress = match event.entity.nbt.get(&"growth".into()) {
            Some(Tag::Int(v)) => (*v).max(0) as u32,
            _ => 0,
        };
        let (next, ripe) = tick(progress, soil.as_deref(), seed.as_deref());
        let mut next = next;
        if ripe {
            let time = host.level(host.dimension()).and_then(|l| l.game_time(&*host)).unwrap_or(0);
            if let Some(crop) = seed.as_deref().and_then(crop_of) {
                let drops: Vec<ItemStack> = harvest(crop, roll_stream(time, pos))
                    .into_iter()
                    .filter_map(|(id, n)| ItemStack::new(host.item_key(&Identifier::parse(id).ok()?)?, n))
                    .collect();
                // all or nothing: the harvest waits for room
                let mut trial = items.clone();
                if drops.iter().all(|d| insert(&*host, &mut trial, d.clone())) {
                    items = trial;
                    next = 0;
                }
            }
        }
        if next == progress && items == event.entity.items {
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        }
        event.entity.nbt.insert("growth".into(), Tag::Int(next as i32));
        event.entity.items = items;
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let key = r.block_key(POT);
        r.block_container(
            &key,
            ModContainer {
                slots: 9,
                faces: std::array::from_fn(|_| (0..9).collect()),
                inputs: vec![SOIL, SEED],
                outputs: OUTPUTS.collect(),
                menu: Some("minecraft:generic_9x1".into()),
            },
        );
        let kind = r.block_entity_type("botany_pot").blocks(&[POT]).build();
        let _ = r.block_entity_part(&kind, Pot);
    }
}
