//! Mama's MerryMaking for Horizon: the interactable holiday candle holders (the Hanukkah
//! menorah and the Kwanzaa kinara).
//!
//! Written from the mod's public description ("special interactable candle holders for Hanukkah
//! and Kwanzaa") and general knowledge of the two customs, never from its code
//! (docs/modpacks.md, "Licences"). How the original's interaction works is not described
//! publicly, so the rule here is an assumption listed in README.md.
//!
//! How it works: each holder has a `candles` state property, the number of candles lit.
//! Using flint and steel on it lights the next candle, one a click, as one more is lit each
//! night; an empty-handed click puts them all out, as vanilla's candles go out.
use std::sync::OnceLock;

use mod_api::{
    BlockKey, BlockPart, BlockState, BlockUpdateFlags, EquipmentSlot, Hand, Host, HostBlockStates,
    InteractionResult, ItemKey, LevelRef, PlayerRef, Registrar, RegistrationContext, ServerHost,
    SoundSource, UseItemOnEvent, UseWithoutItemEvent,
};

/// The holders: block id and how many candles it holds.
pub const HOLDERS: [(&str, u8); 2] = [("merrymaking:menorah", 9), ("merrymaking:kinara", 7)];

/// What a click on a holder does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Light,
    PutOut,
}

/// The candles lit after `action` on a holder of `max` candles with `lit` lit, or None when
/// nothing changes.
pub fn after(lit: u8, max: u8, action: Action) -> Option<u8> {
    match action {
        Action::Light if lit < max => Some(lit + 1),
        Action::PutOut if lit > 0 => Some(0),
        _ => None,
    }
}

static FLINT_AND_STEEL: OnceLock<ItemKey> = OnceLock::new();

/// One holder's behaviour.
struct Holder {
    max: u8,
}

impl Holder {
    fn apply(
        &self,
        host: &mut dyn ServerHost,
        state: BlockState,
        pos: [i32; 3],
        player: u128,
        action: Action,
    ) -> Option<(PlayerRef, LevelRef)> {
        let read: &dyn Host = &*host;
        let player = read.player(player)?;
        let level = player.level(read)?;
        let metadata = read.block_states();
        let lit: u8 = state.property(&metadata, "candles").ok()??.parse().ok()?;
        let next = after(lit, self.max, action)?;
        let new = state.with(&metadata, "candles", &next.to_string()).ok()?;
        let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
        level
            .set_block(host, pos, new, flags)
            .ok()?
            .then_some((player, level))
    }
}

fn centre(pos: [i32; 3]) -> [f64; 3] {
    pos.map(|c| f64::from(c) + 0.5)
}

#[mod_api::horizon_part]
impl BlockPart for Holder {
    /// Flint and steel lights the next candle and wears by one.
    fn use_item_on(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseItemOnEvent<'_>,
    ) -> Option<InteractionResult> {
        let click = event.click;
        let flint = *FLINT_AND_STEEL.get()?;
        if click.stack.map(|s| s.item()) != Some(flint) {
            return None;
        }
        let (player, level) =
            self.apply(host, event.state, click.pos, click.player, Action::Light)?;
        let sound = "minecraft:item.flintandsteel.use";
        level.play_sound(
            host,
            sound,
            centre(click.pos),
            SoundSource::Blocks,
            1.0,
            1.0,
        );
        let creative = player.has_infinite_materials(&*host).unwrap_or(false);
        if !creative && let Some(living) = host.living(player.entity()) {
            let slot = match click.hand {
                Hand::Main => EquipmentSlot::MainHand,
                Hand::Off => EquipmentSlot::OffHand,
            };
            living.hurt_and_break(host, slot, 1);
        }
        Some(InteractionResult::Success)
    }

    /// An empty hand puts every candle out.
    fn use_without_item(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseWithoutItemEvent<'_>,
    ) -> Option<InteractionResult> {
        let click = event.click;
        let (_, level) = self.apply(host, event.state, click.pos, click.player, Action::PutOut)?;
        let sound = "minecraft:block.candle.extinguish";
        level.play_sound(
            host,
            sound,
            centre(click.pos),
            SoundSource::Blocks,
            1.0,
            1.0,
        );
        Some(InteractionResult::Success)
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl mod_api::Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let _ = FLINT_AND_STEEL.set(r.item_key("minecraft:flint_and_steel"));
        for (id, max) in HOLDERS {
            let block: BlockKey = r.block_key(id);
            r.block_part(block, Holder { max });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lighting_adds_one_candle_up_to_the_holders_size() {
        assert_eq!(after(0, 9, Action::Light), Some(1));
        assert_eq!(after(8, 9, Action::Light), Some(9));
        assert_eq!(after(9, 9, Action::Light), None);
        assert_eq!(after(7, 7, Action::Light), None);
    }

    #[test]
    fn putting_out_clears_every_candle() {
        assert_eq!(after(5, 9, Action::PutOut), Some(0));
        assert_eq!(after(0, 7, Action::PutOut), None);
    }
}
