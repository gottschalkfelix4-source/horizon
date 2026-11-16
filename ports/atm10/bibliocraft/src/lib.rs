//! Bibliocraft for Horizon: bookcases and potion shelves.
//!
//! Written from Bibliocraft's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). README.md lists what is ported; the rest of the mod is not.
//!
//! `bibliocraft:<wood>_bookcase` (18 slots, books only) and `<wood>_potion_shelf` (9 slots,
//! bottles only) are planks-template blocks with a container in vanilla's generic 9x2 and 9x1
//! menus. A `menu_slot_rules` part on those two menus refuses other items in a shelf's slots when
//! a player clicks.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockPart, Compound, Hooks, Host, LevelSide, MenuContainer,
    MenuPart, MenuSlotRulesEvent, ModContainer, Registrar, RegistrationContext,
};

pub mod rules;
use rules::*;

struct Shelves;

#[mod_api::horizon_part]
impl BlockPart for Shelves {}

#[mod_api::horizon_part]
impl BlockEntityPart for Shelves {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        shelf_of(event.block).map(|_| Compound::new())
    }
}

/// Which shelf, if any, the menu being clicked belongs to.
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
        let Some(shelf) = shelf_of(&block.id(host)) else { return };
        if event.slot < shelf.slots() && !shelf.accepts(&event.stack.item().id(host)) {
            event.may_place = false;
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for shelf in Shelf::ALL {
            let ids: Vec<String> = WOODS.iter().map(|w| block_id(w, shelf)).collect();
            let every: Vec<usize> = (0..shelf.slots()).collect();
            for id in &ids {
                let key = r.block_key(id);
                r.block_container(
                    &key,
                    ModContainer {
                        slots: shelf.slots(),
                        faces: std::array::from_fn(|_| every.clone()),
                        inputs: every.clone(),
                        outputs: every.clone(),
                        menu: Some(shelf.menu().into()),
                    },
                );
                r.block_part(key, Shelves);
            }
            let names: Vec<&str> = ids.iter().map(String::as_str).collect();
            let kind = r.block_entity_type(shelf.name()).blocks(&names).build();
            let _ = r.block_entity_part(&kind, Shelves);
            r.menu_part(shelf.menu(), Rules);
        }
    }
}
