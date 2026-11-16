//! Repeatable Trial Vaults for Horizon: a trial vault remembers every player it rewarded and
//! never opens for them again; with this mod, using a key on it (any item in
//! `#repeatable_trial_vaults:can_reset_trial_vaults`, by default the two trial keys) makes the
//! vault forget that player, so it opens for them once more.
//!
//! Written from the mod's public description, never from its code (docs/modpacks.md,
//! "Licences"). What the port does, and what a harness against the NeoForge original still has
//! to confirm, is in README.md.
//!
//! How it works: `use_item_on_block` sees the click before the vault does. When the player is in
//! the vault's `server_data.rewarded_players`, they are taken out of it and the vault is set
//! `active` again. If the item is the vault's own key, vanilla then goes on and takes it to
//! unlock the vault: one key, one more reward. Another item of the tag is consumed for the
//! reset alone, and the player unlocks with a key as usual.
use std::sync::OnceLock;

use mod_api::{
    BlockKey, Compound, Flow, Hooks, HostSide, HostTransaction, Identifier, InteractionResult,
    ItemPlace, ItemResource, ItemStack, Registrar, RegistrationContext, Tag, UseItemOnBlockEvent,
    UsePhase,
};

const RESET_TAG: &str = "repeatable_trial_vaults:can_reset_trial_vaults";
const DEFAULT_KEY: &str = "minecraft:trial_key";

static VAULT: OnceLock<BlockKey> = OnceLock::new();

/// A UUID as NBT writes it: four ints, most significant first.
pub fn uuid_ints(uuid: u128) -> [i32; 4] {
    [
        (uuid >> 96) as u32 as i32,
        (uuid >> 64) as u32 as i32,
        (uuid >> 32) as u32 as i32,
        uuid as u32 as i32,
    ]
}

/// Takes `player` out of a vault's saved `server_data.rewarded_players`: true when they were
/// in it.
pub fn forget_player(data: &mut Compound, player: u128) -> bool {
    let ints = uuid_ints(player);
    let Some(Tag::Compound(server)) = data.get_mut(&"server_data".into()) else {
        return false;
    };
    let Some(Tag::List(rewarded)) = server.get_mut(&"rewarded_players".into()) else {
        return false;
    };
    let before = rewarded.len();
    rewarded.retain(|t| !matches!(t, Tag::IntArray(v) if v.as_slice() == ints));
    before != rewarded.len()
}

/// The vault's key item (`config.key_item.id`), vanilla's trial key when the vault names none.
pub fn key_item(data: &Compound) -> String {
    let id = match data.get(&"config".into()) {
        Some(config) => match config.get("key_item").and_then(|k| k.get("id")) {
            Some(Tag::String(s)) => Some(s.to_string_lossy().to_string()),
            _ => None,
        },
        None => None,
    };
    match id {
        Some(id) if id.contains(':') => id,
        Some(id) => format!("minecraft:{id}"),
        None => DEFAULT_KEY.to_owned(),
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let _ = VAULT.set(r.block_key("minecraft:vault"));
    }

    fn use_item_on_block(&self, side: HostSide, event: &mut UseItemOnBlockEvent<'_>) -> Flow {
        let HostSide::Server(host) = side else {
            return Flow::Continue;
        };
        let click = event.click;
        if event.phase != UsePhase::Block {
            return Flow::Continue;
        }
        let Some(stack) = click.stack else {
            return Flow::Continue;
        };
        let Some(vault) = VAULT.get() else {
            return Flow::Continue;
        };
        let Some(level) = host.level(&click.dimension.key().to_string()) else {
            return Flow::Continue;
        };
        if level.block(host, click.pos) != Some(*vault) {
            return Flow::Continue;
        }
        // A vault busy unlocking or ejecting is left alone.
        let idle = level
            .block_property(host, click.pos, "vault_state")
            .is_some_and(|s| s == "active" || s == "inactive");
        if !idle {
            return Flow::Continue;
        }
        let in_tag = Identifier::parse(RESET_TAG)
            .ok()
            .and_then(|id| host.tags().key(&id))
            .and_then(|tag| host.tags().is_in_tag(stack.item(), tag));
        if in_tag != Some(true) {
            return Flow::Continue;
        }
        let Some(entity) = host.block_entity(level, click.pos) else {
            return Flow::Continue;
        };
        let Some(mut data) = entity.data(host) else {
            return Flow::Continue;
        };
        if !forget_player(&mut data, click.player) {
            return Flow::Continue;
        }
        let key = key_item(&data);
        if !matches!(entity.replace_data(host, data), Ok(true)) {
            return Flow::Continue;
        }
        // Inactive for this player until now; vanilla only takes a key on an active vault.
        host.set_block_property(click.pos, "vault_state", "active");
        let mut key_nbt = Compound::new();
        key_nbt.insert("id".into(), Tag::String(key.as_str().into()));
        key_nbt.insert("count".into(), Tag::Int(1));
        let key_stack = ItemStack::from_nbt(&*host, &key_nbt);
        if key_stack.is_some_and(|k| k.item() == stack.item()) {
            // Vanilla takes this key and unlocks the vault.
            return Flow::Continue;
        }
        let hand = ItemPlace::PlayerHand {
            player: click.player,
            hand: click.hand,
        };
        let resource = ItemResource::of(stack);
        let _ = host.transaction(|host, tx| match hand.extract(host, tx, &resource, 1) {
            1 => Ok(()),
            _ => Err(()),
        });
        *event.cancellation_result = InteractionResult::Success;
        Flow::Cancel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid_tag(uuid: u128) -> Tag {
        Tag::IntArray(uuid_ints(uuid).to_vec())
    }

    #[test]
    fn uuids_split_most_significant_first() {
        let uuid = 0x0000_0001_FFFF_FFFF_8000_0000_0000_0002u128;
        assert_eq!(uuid_ints(uuid), [1, -1, i32::MIN, 2]);
    }

    #[test]
    fn only_the_rewarded_player_is_forgotten() {
        let (me, other) = (7u128 << 100 | 3, 42u128);
        let mut server = Compound::new();
        server.insert(
            "rewarded_players".into(),
            Tag::List(vec![uuid_tag(other), uuid_tag(me)]),
        );
        let mut data = Compound::new();
        data.insert("server_data".into(), Tag::Compound(server));

        assert!(forget_player(&mut data, me));
        let Some(Tag::Compound(server)) = data.get(&"server_data".into()) else {
            panic!("server_data gone");
        };
        assert_eq!(
            server.get(&"rewarded_players".into()),
            Some(&Tag::List(vec![uuid_tag(other)]))
        );
        assert!(!forget_player(&mut data, me));
        assert!(!forget_player(&mut Compound::new(), me));
    }

    #[test]
    fn the_key_item_comes_from_the_vault_config() {
        let mut key = Compound::new();
        key.insert("id".into(), Tag::String("ominous_trial_key".into()));
        let mut config = Compound::new();
        config.insert("key_item".into(), Tag::Compound(key));
        let mut data = Compound::new();
        data.insert("config".into(), Tag::Compound(config));
        assert_eq!(key_item(&data), "minecraft:ominous_trial_key");
        assert_eq!(key_item(&Compound::new()), "minecraft:trial_key");
    }
}
