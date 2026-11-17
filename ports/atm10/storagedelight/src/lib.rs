//! Storage Delight for Horizon: a pantry and a fridge that keep food.
//!
//! Written from Storage Delight's public listing, never from its code (docs/modpacks.md,
//! "Licences"). README.md says how little of the original could be recalled.
//!
//! `storagedelight:pantry` (27 slots, vanilla's 9x3 menu) and `fridge` (18 slots, 9x2) are
//! containers whose slots refuse, on a player's click, anything that is not food
//! (the fridge also takes milk and bottled drinks): a `menu_slot_rules` part reads the item's
//! default `minecraft:food` component.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockPart, Compound, Hooks, Host, LevelSide, MenuContainer,
    MenuPart, MenuSlotRulesEvent, ModContainer, Registrar, RegistrationContext,
};

pub mod rules;
use rules::*;

struct Stores;

#[mod_api::horizon_part]
impl BlockPart for Stores {}

#[mod_api::horizon_part]
impl BlockEntityPart for Stores {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        Store::of_block(event.block).map(|_| Compound::new())
    }
}

struct Rules;

#[mod_api::horizon_part]
impl MenuPart for Rules {
    fn menu_slot_rules(&self, host: &dyn Host, event: &mut MenuSlotRulesEvent<'_>) {
        if event.container != MenuContainer::Main {
            return;
        }
        let Some(pos) = event.context.pos else { return };
        let Some(level) = host.level(host.dimension()) else { return };
        let Some(block) = level.block(host, pos) else { return };
        let Some(store) = Store::of_block(&block.id(host)) else { return };
        if event.slot >= store.slots() {
            return;
        }
        let item = event.stack.item();
        let is_food = host.item_default_component(item, "minecraft:food").is_some();
        if !store.accepts(&item.id(host), is_food) {
            event.may_place = false;
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for store in Store::ALL {
            let id = format!("storagedelight:{}", store.name());
            let key = r.block_key(&id);
            let every: Vec<usize> = (0..store.slots()).collect();
            r.block_container(
                &key,
                ModContainer {
                    slots: store.slots(),
                    faces: std::array::from_fn(|_| every.clone()),
                    inputs: every.clone(),
                    outputs: every,
                    menu: Some(store.menu().into()),
                    slot_limit: 0,
                },
            );
            let kind = r.block_entity_type(store.name()).blocks(&[id.as_str()]).build();
            let _ = r.block_entity_part(&kind, Stores);
            r.menu_part(store.menu(), Rules);
        }
    }
}
