//! Pocket Storage for Horizon: small, medium and large storage blocks that sort on a sneak-use.
//!
//! Written from Pocket Storage's public listing, never from its code (docs/modpacks.md,
//! "Licences"). README.md says what the port assumes the original is, and what is missing.
//!
//! `pocketstorage:pocket_pouch`, `pocket_chest` and `pocket_vault` are chest-template blocks with a
//! container of 9, 27 or 54 slots in the vanilla menu of that size. Using one while sneaking
//! with an empty hand merges and sorts its stacks.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockPart, Compound, Hooks, HostTransaction,
    InteractionResult, ItemResource, ItemStack, LevelSide, ModContainer, Registrar,
    RegistrationContext, ServerHost, UseWithoutItemEvent,
};

pub mod rules;
use rules::*;

struct Pocket;

fn slots_of(block: &str) -> Option<usize> {
    let name = block.strip_prefix("pocketstorage:")?;
    TIERS.iter().find(|(n, _, _)| *n == name).map(|(_, s, _)| *s)
}

#[mod_api::horizon_part]
impl BlockPart for Pocket {
    /// Sneak-use with an empty hand sorts the container; a plain use opens the menu as usual.
    fn use_without_item(&self, host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
        let click = event.click;
        let level = host.level(host.dimension())?;
        let slots = slots_of(&level.block(&*host, click.pos)?.id(&*host))?;
        let player = host.player(click.player)?;
        if !player.entity().is_shift_key_down(&*host).unwrap_or(false) {
            return None;
        }
        let entity = host.block_entity(level, click.pos)?;
        let before: Vec<Option<(ItemResource, u32)>> = (0..slots as u32)
            .map(|i| {
                entity
                    .item_slot(&*host, i)
                    .ok()
                    .flatten()
                    .map(|s| (ItemResource::of(&s), s.count()))
            })
            .collect();
        let max_of = |host: &dyn mod_api::Host, k: &ItemResource| host.max_stack_size(k.item()).unwrap_or(64);
        let sorted = compact(
            before,
            slots,
            |k| k.item().id(&*host),
            |k| max_of(&*host, k),
        );
        let stacks: Vec<Option<ItemStack>> = sorted
            .into_iter()
            .map(|s| s.and_then(|(k, n)| k.to_stack(n)))
            .collect();
        let _ = host.transaction(|host, tx| {
            for (i, stack) in stacks.into_iter().enumerate() {
                let _ = entity.set_item_slot(host, tx, i as u32, stack);
            }
            Ok::<(), ()>(())
        });
        let _ = entity.set_changed(host);
        Some(InteractionResult::SuccessServer)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for Pocket {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        slots_of(event.block).map(|_| Compound::new())
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let ids: Vec<String> = TIERS.iter().map(|(n, _, _)| format!("pocketstorage:{n}")).collect();
        let names: Vec<&str> = ids.iter().map(String::as_str).collect();
        for ((_, slots, menu), id) in TIERS.iter().zip(&ids) {
            let key = r.block_key(id);
            let every: Vec<usize> = (0..*slots).collect();
            r.block_container(
                &key,
                ModContainer {
                    slots: *slots,
                    faces: std::array::from_fn(|_| every.clone()),
                    inputs: every.clone(),
                    outputs: every.clone(),
                    menu: Some((*menu).into()),
                },
            );
        }
        let kind = r.block_entity_type("pocket").blocks(&names).build();
        let _ = r.block_entity_part(&kind, Pocket);
    }
}
