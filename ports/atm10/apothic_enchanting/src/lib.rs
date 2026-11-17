//! Apothic Enchanting for Horizon: enchanting shelves and the library.
//!
//! Written from Apothic Enchanting's observed behaviour and public description, never from its
//! code (docs/modpacks.md, "Licences"). README.md lists what is ported and what is not.
//!
//! The four shelves add to an enchanting table's power through `enchant_power_bonus`: each call
//! (one per shelf cell the game checks) adds its share of the table's eterna, which is worked out
//! from every mod shelf in the ring around the table (`rules::eterna`). The game caps a table's
//! power at 15, so the shelves reach vanilla's level 30 and no further (see README.md).
//! The library is an 18-slot container that takes enchanted books only.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockPart, Compound, EnchantPowerBonusEvent, LevelSide,
    MenuContainer, MenuPart, MenuSlotRulesEvent, ModContainer, Registrar, RegistrationContext,
    Hooks, Host,
};

pub mod rules;
use rules::*;

const LIBRARY: &str = "apothic_enchanting:library";
const MENU: &str = "minecraft:generic_9x2";
const SLOTS: usize = 18;

struct Shelves;

#[mod_api::horizon_part]
impl BlockPart for Shelves {
    fn enchant_power_bonus(&self, host: &dyn Host, event: &mut EnchantPowerBonusEvent) {
        let Some(level) = host.level(host.dimension()) else { return };
        let table = [event.table_pos.x, event.table_pos.y, event.table_pos.z];
        let found: Vec<&Shelf> = ring()
            .into_iter()
            .filter_map(|d| {
                let at = [table[0] + d[0], table[1] + d[1], table[2] + d[2]];
                let id = level.block(host, at)?.id(host);
                shelf_of(&id)
            })
            .collect();
        if found.is_empty() {
            return;
        }
        event.power += eterna(found.iter().copied()) / found.len() as f32;
    }
}

struct Library;

#[mod_api::horizon_part]
impl BlockPart for Library {}

#[mod_api::horizon_part]
impl BlockEntityPart for Library {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == LIBRARY).then(Compound::new)
    }
}

struct Books;

#[mod_api::horizon_part]
impl MenuPart for Books {
    fn menu_slot_rules(&self, host: &dyn Host, event: &mut MenuSlotRulesEvent<'_>) {
        if event.container != MenuContainer::Main || event.slot >= SLOTS {
            return;
        }
        let Some(pos) = event.context.pos else { return };
        let Some(level) = host.level(host.dimension()) else { return };
        if level.block(host, pos).is_some_and(|b| b.id(host) == LIBRARY)
            && event.stack.item().id(host) != "minecraft:enchanted_book"
        {
            event.may_place = false;
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for shelf in SHELVES {
            let key = r.block_key(&format!("apothic_enchanting:{}", shelf.name));
            r.block_part(key, Shelves);
        }
        let library = r.block_key(LIBRARY);
        let every: Vec<usize> = (0..SLOTS).collect();
        r.block_container(
            &library,
            ModContainer {
                slots: SLOTS,
                faces: std::array::from_fn(|_| every.clone()),
                inputs: every.clone(),
                outputs: every,
                menu: Some(MENU.into()),
            },
        );
        let kind = r.block_entity_type("library").blocks(&[LIBRARY]).build();
        let _ = r.block_entity_part(&kind, Library);
        r.menu_part(MENU, Books);
    }
}
