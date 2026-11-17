//! RFToolsStorage for Horizon: the Modular Storage and its Storage Modules.
//!
//! Written from RFToolsStorage's observed behaviour and public description, never from its
//! code (docs/modpacks.md, "Licences"). What is ported, and what a check against the NeoForge
//! original still has to confirm, is in README.md.
//!
//! A Modular Storage holds up to three Storage Modules; their sizes add up to the number of
//! stack slots of one big storage ([`vault`]). Items put in the input slot are stored; the output
//! slot is kept filled from the selected kind. Hoppers use the same two slots.
pub mod vault;

use std::sync::OnceLock;

use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockKey, BlockPart, Compound, Hooks, Host, InteractionResult,
    ItemStack, LevelSide, ModContainer, Registrar, RegistrationContext, ServerHost, Tag, Text,
    UseWithoutItemEvent,
};

use crate::vault::{Entry, Vault};

pub const MODULE_SLOTS: usize = 3;
pub const INPUT: usize = 3;
pub const OUTPUT: usize = 4;
pub const SLOTS: usize = 5;

/// Stack slots a module of the item `id` adds (0 for anything else). Tier 1 to 3.
pub fn module_slots(id: &str) -> u64 {
    match id {
        "rftoolsstorage:storage_module0" => 300,
        "rftoolsstorage:storage_module1" => 1_000,
        "rftoolsstorage:storage_module2" => 3_000,
        _ => 0,
    }
}

static BLOCK: OnceLock<BlockKey> = OnceLock::new();

fn is_ours(block: &str) -> bool {
    block == "rftoolsstorage:modular_storage"
}

fn long(nbt: &Compound, key: &str) -> i64 {
    match nbt.get(&key.into()) {
        Some(Tag::Long(v)) => *v,
        Some(Tag::Int(v)) => i64::from(*v),
        _ => 0,
    }
}

fn save_vault(host: &dyn Host, nbt: &mut Compound, vault: &Vault<ItemStack>) {
    let list = vault
        .entries
        .iter()
        .filter_map(|e| {
            let mut stack = e.kind.to_nbt(host)?;
            stack.insert("stored".into(), Tag::Long(e.count as i64));
            Some(Tag::Compound(stack))
        })
        .collect();
    nbt.insert("vault".into(), Tag::List(list));
}

fn load_vault(host: &dyn Host, nbt: &Compound) -> Vault<ItemStack> {
    let mut vault = Vault::default();
    if let Some(Tag::List(list)) = nbt.get(&"vault".into()) {
        for tag in list {
            let Tag::Compound(c) = tag else { continue };
            let Some(kind) = ItemStack::from_nbt(host, c) else { continue };
            let max = host.max_stack_size(kind.item()).unwrap_or(64);
            let count = long(c, "stored").max(0) as u64;
            if count > 0 {
                vault.entries.push(Entry { kind, max, count });
            }
        }
    }
    vault
}

/// One stack of what is stored, `n` at most.
fn prototype(stack: &ItemStack) -> ItemStack {
    let mut p = stack.clone();
    let _ = p.set_count(1);
    p
}

fn tick(host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    if event.entity.items.len() < SLOTS {
        event.entity.items.resize(SLOTS, None);
    }
    let capacity: u64 = event.entity.items[..MODULE_SLOTS]
        .iter()
        .flatten()
        .map(|s| module_slots(&s.item().id(host)))
        .sum();
    let has_input = event.entity.items[INPUT].is_some();
    let want_output = event.entity.items[OUTPUT].as_ref().is_none_or(|s| {
        s.count() < host.max_stack_size(s.item()).unwrap_or(64)
    });
    if !has_input && !want_output {
        *event.reply = BlockEntityTickReply::Unchanged;
        return;
    }
    let mut vault = load_vault(host, &event.entity.nbt);
    let before = vault.clone();
    let mut changed = false;

    if let Some(stack) = event.entity.items[INPUT].clone() {
        let max = host.max_stack_size(stack.item()).unwrap_or(64);
        let proto = prototype(&stack);
        let taken = vault.insert(&proto, max, stack.count(), capacity, |k| k.is_stackable_with(&stack));
        if taken > 0 {
            event.entity.items[INPUT] = stack.shrink(taken);
            changed = true;
        }
    }

    let selected = long(&event.entity.nbt, "selected").max(0) as usize;
    if want_output && !vault.entries.is_empty() {
        let index = selected.min(vault.entries.len() - 1);
        let kind = vault.entries[index].kind.clone();
        let max = vault.entries[index].max;
        let (have, fits) = match &event.entity.items[OUTPUT] {
            None => (0, true),
            Some(held) => (held.count(), held.is_stackable_with(&kind)),
        };
        if fits {
            let out = vault.take(index, max.saturating_sub(have));
            if out > 0 {
                match event.entity.items[OUTPUT].as_mut() {
                    Some(held) => {
                        let _ = held.grow(out);
                    }
                    None => {
                        let mut s = kind;
                        let _ = s.set_count(out);
                        event.entity.items[OUTPUT] = Some(s);
                    }
                }
                changed = true;
            }
        }
    }

    if vault != before {
        save_vault(host, &mut event.entity.nbt, &vault);
    }
    if !changed {
        *event.reply = BlockEntityTickReply::Unchanged;
    }
}

struct ModularStorage;

#[mod_api::horizon_part]
impl BlockPart for ModularStorage {
    /// Sneak-using with an empty hand selects the next kind for the output slot.
    fn use_without_item(&self, host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
        if Some(&event.state.block()) != BLOCK.get() {
            return None;
        }
        let click = event.click;
        let sneaking = host.player(click.player)?.is_secondary_use_active(host)?;
        if !sneaking {
            return None;
        }
        let level = host.level(host.dimension())?;
        let entity = host.block_entity(level, click.pos)?;
        let nbt = entity.data(host)?;
        let vault = load_vault(host, &nbt);
        let player = host.player(click.player)?;
        if vault.entries.is_empty() {
            let _ = player.action_bar(host, Text::translate("rftoolsstorage.empty", Vec::new()));
            return Some(InteractionResult::SuccessServer);
        }
        let next = (long(&nbt, "selected").max(0) as usize + 1) % vault.entries.len();
        let name = vault.entries[next].kind.item().id(host);
        let mut patch = Compound::new();
        patch.insert("selected".into(), Tag::Int(next as i32));
        let _ = entity.merge_data(host, patch);
        let _ = player.action_bar(host, Text::translate("rftoolsstorage.selected", vec![Text::literal(&name)]));
        Some(InteractionResult::SuccessServer)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for ModularStorage {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        is_ours(event.block).then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if is_ours(event.block) {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        tick(host, event);
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let key = r.block_key("rftoolsstorage:modular_storage");
        let _ = BLOCK.set(key);
        r.block_container(
            &key,
            ModContainer {
                slots: SLOTS,
                faces: std::array::from_fn(|_| vec![INPUT, OUTPUT]),
                inputs: vec![0, 1, 2, INPUT],
                outputs: vec![OUTPUT],
                menu: Some("minecraft:hopper".into()),
            },
        );
        let kind = r
            .block_entity_type("modular_storage")
            .blocks(&["rftoolsstorage:modular_storage"])
            .build();
        let _ = r.block_entity_part(&kind, ModularStorage);
    }
}
