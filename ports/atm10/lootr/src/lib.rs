//! Lootr for Horizon: a loot chest every player opens for themselves.
//!
//! Written from Lootr's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). README.md lists what is ported and what a check against the
//! NeoForge original still has to confirm.
//!
//! `lootr:loot_chest` keeps one inventory per player in its saved `players` compound (UUID, then
//! its stacks by slot). Its container is the vanilla 9x3 menu, but hoppers and pipes cannot reach
//! it: when a player opens it, `player_container_open` fills the 27 slots with that player's
//! saved stacks, or rolls the chest's `LootTable` for them the first time, and
//! `player_container_close` saves the slots back and empties them. Only one player has it open at
//! a time.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockPart, Compound, ContainerOpenedEvent, Flow, Hooks, HostTransaction,
    ItemStack, LevelRef, LevelSide, ModContainer, PlayerContainerCloseEvent,
    PlayerContainerOpenEvent, Registrar, RegistrationContext, ServerHost, Tag,
};

pub mod rules;
use rules::*;

const CHEST: &str = "lootr:loot_chest";
const MENU: &str = "minecraft:generic_9x3";

fn hex(player: u128) -> String {
    format!("{player:032x}")
}

fn is_chest(host: &dyn mod_api::Host, level: LevelRef, pos: [i32; 3]) -> bool {
    level.block(host, pos).is_some_and(|b| b.id(host) == CHEST)
}

/// The saved stacks of one player: (slot, stack).
fn saved(host: &dyn mod_api::Host, players: &Compound, player: u128) -> Option<Vec<(usize, ItemStack)>> {
    let Some(Tag::List(list)) = players.get(&hex(player).as_str().into()) else {
        return None;
    };
    Some(
        list.iter()
            .filter_map(|t| {
                let Tag::Compound(c) = t else { return None };
                let Some(Tag::Int(slot)) = c.get(&"slot".into()) else { return None };
                let Some(Tag::Compound(item)) = c.get(&"item".into()) else { return None };
                Some((*slot as usize, ItemStack::from_nbt(host, item)?))
            })
            .collect(),
    )
}

fn players_of(data: &Compound) -> Compound {
    match data.get(&"players".into()) {
        Some(Tag::Compound(c)) => c.clone(),
        _ => Compound::new(),
    }
}

struct Chest;

#[mod_api::horizon_part]
impl BlockPart for Chest {}

#[mod_api::horizon_part]
impl BlockEntityPart for Chest {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == CHEST).then(Compound::new)
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let key = r.block_key(CHEST);
        r.block_container(
            &key,
            ModContainer {
                slots: SLOTS,
                faces: std::array::from_fn(|_| Vec::new()),
                inputs: Vec::new(),
                outputs: Vec::new(),
                menu: Some(MENU.into()),
                slot_limit: 0,
            },
        );
        let kind = r.block_entity_type("loot_chest").blocks(&[CHEST]).build();
        let _ = r.block_entity_part(&kind, Chest);
    }

    /// A second player is turned away while one has the chest open.
    fn container_opened(&self, host: &mut dyn ServerHost, event: &mut ContainerOpenedEvent<'_>) -> Flow {
        let Some(pos) = event.pos else { return Flow::Continue };
        let Some(level) = host.level(host.dimension()) else { return Flow::Continue };
        if !is_chest(&*host, level, pos) {
            return Flow::Continue;
        }
        let busy = host
            .block_entity(level, pos)
            .and_then(|e| e.viewers(&*host))
            .is_some_and(|v| !v.is_empty());
        if busy {
            if let Some(player) = host.player(event.player) {
                let line = mod_api::Text::translate("message.lootr.in_use", vec![]);
                let _ = player.action_bar(host, line);
            }
            return Flow::Cancel;
        }
        Flow::Continue
    }

    fn player_container_open(&self, host: &mut dyn ServerHost, event: &mut PlayerContainerOpenEvent<'_>) {
        let (Some(pos), true) = (event.pos, event.menu == MENU) else { return };
        let Some(level) = host.level(host.dimension()) else { return };
        if !is_chest(&*host, level, pos) {
            return;
        }
        let Some(entity) = host.block_entity(level, pos) else { return };
        let Some(data) = entity.data(&*host) else { return };
        let players = players_of(&data);
        let table = match data.get(&"LootTable".into()) {
            Some(Tag::String(s)) => Some(s.to_string_lossy()),
            _ => None,
        };
        let stacks: Vec<(usize, ItemStack)> = match opening(players.contains_key(&hex(event.player).as_str().into()), table.as_deref()) {
            Opening::Restore => saved(&*host, &players, event.player).unwrap_or_default(),
            Opening::Empty => Vec::new(),
            Opening::Roll(table) => {
                let origin = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5];
                let rolled = level
                    .roll_loot(host, &table, origin, None, None)
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                let slots = scatter(rolled.len(), SLOTS, seed(event.player, pos));
                slots.into_iter().zip(rolled).collect()
            }
        };
        let _ = host.transaction(|host, tx| {
            for slot in 0..SLOTS {
                let stack = stacks.iter().find(|(s, _)| *s == slot).map(|(_, st)| st.clone());
                let _ = entity.set_item_slot(host, tx, slot as u32, stack);
            }
            Ok::<(), ()>(())
        });
    }

    fn player_container_close(&self, host: &mut dyn ServerHost, event: &mut PlayerContainerCloseEvent<'_>) {
        let (Some(pos), true) = (event.pos, event.menu == MENU) else { return };
        let Some(level) = host.level(host.dimension()) else { return };
        if !is_chest(&*host, level, pos) {
            return;
        }
        let Some(entity) = host.block_entity(level, pos) else { return };
        let Some(data) = entity.data(&*host) else { return };
        let mut list = Vec::new();
        for slot in 0..SLOTS as u32 {
            let Ok(Some(stack)) = entity.item_slot(&*host, slot) else { continue };
            let Some(item) = stack.to_nbt(&*host) else { continue };
            let mut c = Compound::new();
            c.insert("slot".into(), Tag::Int(slot as i32));
            c.insert("item".into(), Tag::Compound(item));
            list.push(Tag::Compound(c));
        }
        let mut players = players_of(&data);
        players.insert(hex(event.player).as_str().into(), Tag::List(list));
        let mut patch = Compound::new();
        patch.insert("players".into(), Tag::Compound(players));
        let _ = entity.merge_data(host, patch);
        // the slots belong to nobody until the next player opens the chest
        let _ = host.transaction(|host, tx| {
            for slot in 0..SLOTS as u32 {
                let _ = entity.set_item_slot(host, tx, slot, None);
            }
            Ok::<(), ()>(())
        });
    }
}
