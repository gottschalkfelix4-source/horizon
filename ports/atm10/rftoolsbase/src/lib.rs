//! RFToolsBase for Horizon: the pieces every RFTools mod builds on.
//!
//! Written from RFToolsBase's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What is ported and what still has to be checked against the
//! NeoForge original is in README.md.
//!
//! - Machine Frame and Machine Base: crafting blocks, no behaviour.
//! - Dimensional Shard ores (overworld, nether, end) and the shard they drop.
//! - Filter Module: the item filter RFTools machines and XNet connectors read ([`filter`]).
//! - Information Screen: reads the energy of the block it sits on and keeps the line it shows
//!   ([`info`]); using it cycles text, percentage and rate.
pub mod energy;
pub mod filter;
pub mod info;

use std::sync::OnceLock;

use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockKey, BlockPart, Compound, Hooks, Host, InteractionResult,
    ItemStack, LevelSide, Registrar, RegistrationContext, ServerHost, Tag, UseWithoutItemEvent,
};

use crate::filter::{Filter, ItemFacts};
use crate::info::{Mode, Reading, SAMPLE_TICKS};

/// The key of `custom_data` a filter module keeps its settings under.
pub const FILTER_KEY: &str = "filter";

/// A filter module's settings, from the stack's `minecraft:custom_data`.
pub fn filter_of(stack: &ItemStack) -> Filter {
    let data = stack
        .components()
        .and_then(|c| c.as_nbt().get(&"minecraft:custom_data".into()).cloned());
    match data {
        Some(Tag::Compound(data)) => match data.get(&FILTER_KEY.into()) {
            Some(Tag::Compound(f)) => Filter::load(f),
            _ => Filter::default(),
        },
        _ => Filter::default(),
    }
}

/// The facts the filter compares, for a stack in the game. The game lists no item's tags, so
/// `tags` holds the tags of `filter`'s ghost items that the stack is in.
pub fn facts_of(host: &dyn Host, stack: &ItemStack, filter: &Filter) -> ItemFacts {
    let tags_api = host.tags();
    let mut tags: Vec<String> = Vec::new();
    for tag in filter.items.iter().flat_map(|g| g.tags.iter()) {
        if tags.contains(tag) {
            continue;
        }
        let Ok(id) = mod_api::Identifier::parse(tag) else {
            continue;
        };
        if tags_api
            .key(&id)
            .and_then(|key| tags_api.is_in_tag(stack.item(), key))
            .unwrap_or(false)
        {
            tags.push(tag.clone());
        }
    }
    ItemFacts {
        id: stack.item().id(host),
        damage: stack.damage(),
        components: stack.components().map(|c| c.as_nbt().clone()).unwrap_or_default(),
        tags,
    }
}

static SCREEN: OnceLock<BlockKey> = OnceLock::new();

fn is_screen(block: &str) -> bool {
    block == "rftoolsbase:information_screen"
}

struct InformationScreen;

#[mod_api::horizon_part]
impl BlockPart for InformationScreen {
    /// Using the screen with an empty hand shows the next mode.
    fn use_without_item(&self, host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
        if Some(&event.state.block()) != SCREEN.get() {
            return None;
        }
        let level = host.level(host.dimension())?;
        let entity = host.block_entity(level, event.click.pos)?;
        let mut reading = Reading::load(&entity.data(host)?);
        reading.mode = reading.mode.next();
        let mut patch = Compound::new();
        reading.save(&mut patch);
        let _ = entity.merge_data(host, patch);
        host.block_entity_sync(event.click.pos);
        Some(InteractionResult::Success)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for InformationScreen {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        is_screen(event.block).then(|| {
            let mut nbt = Compound::new();
            Reading { mode: Mode::Text, ..Default::default() }.save(&mut nbt);
            nbt
        })
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if is_screen(event.block) {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let timer = match event.entity.nbt.get(&"timer".into()) {
            Some(Tag::Int(t)) => *t,
            _ => 0,
        } + 1;
        if timer < SAMPLE_TICKS {
            event.entity.nbt.insert("timer".into(), Tag::Int(timer));
            return;
        }
        event.entity.nbt.insert("timer".into(), Tag::Int(0));
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        let pos = event.pos;
        let now = energy::ALL_FACES
            .iter()
            .find_map(|&dir| energy::read(host, level, pos, dir))
            .map(|(a, c)| (i64::from(a), i64::from(c)));
        let before = Reading::load(&event.entity.nbt);
        let after = before.next(now);
        after.save(&mut event.entity.nbt);
        if after.line() != before.line() {
            host.block_entity_sync(pos);
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let screen = r.block_key("rftoolsbase:information_screen");
        let _ = SCREEN.set(screen);
        let kind = r
            .block_entity_type("information_screen")
            .blocks(&["rftoolsbase:information_screen"])
            .build();
        let _ = r.block_entity_part(&kind, InformationScreen);
    }
}
