//! Shrink for Horizon, first slice: the Shrinking Device.
//!
//! Written from Shrink's public description and from how the device plays, never from its code
//! (docs/modpacks.md, "Licences"). What is from memory and still has to be checked is listed in
//! README.md.
//!
//! Using `shrink:shrinking_device` in the air shrinks its user, and using it again brings them
//! back to full size. Using it on a mob does the same to the mob. Shrinking is a permanent
//! modifier `shrink:shrunk` on vanilla's `minecraft:scale` attribute, so it is saved with the
//! entity, survives relogging, and the game's own scale handling (hitbox, eye height, step height,
//! reach, fall damage) follows it, as it does for Shrink on 1.21.
use std::sync::OnceLock;

use mod_api::{
    AttributeModifier, AttributeOperation, HostSide, Identifier, InteractionResult,
    ItemInteractMobEvent, ItemPart, LivingRef, ModifierLifetime, Registrar, RegistrationContext,
    ServerHost, UseItemEvent,
};

/// The attribute Shrink changes.
pub const SCALE: &str = "minecraft:scale";
/// The scale a shrunk entity takes (Shrink's default, from memory).
pub const SHRUNK_SCALE: f64 = 0.1;
/// The modifier's id on the scale attribute.
const MODIFIER: &str = "shrink:shrunk";

fn modifier_id() -> &'static Identifier {
    static ID: OnceLock<Identifier> = OnceLock::new();
    ID.get_or_init(|| Identifier::parse(MODIFIER).expect("a valid id"))
}

/// The amount of an `AddMultipliedTotal` modifier that makes the attribute `scale` times what it
/// was: the total is multiplied by one plus the amount.
pub fn total_multiplier_amount(scale: f64) -> f64 {
    scale - 1.0
}

/// Whether the entity carrying `modifiers` on its scale attribute is shrunk by this mod.
pub fn is_shrunk(modifiers: &[AttributeModifier]) -> bool {
    modifiers.iter().any(|m| m.id.to_string() == MODIFIER)
}

/// What one use of the device does to an entity that is (`true`) or isn't shrunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    Shrink,
    Restore,
}

pub fn toggle(shrunk: bool) -> Toggle {
    if shrunk {
        Toggle::Restore
    } else {
        Toggle::Shrink
    }
}

/// Shrink `living`, or bring it back to size: true when its scale changed.
fn use_device(host: &mut dyn ServerHost, living: LivingRef) -> bool {
    let Some(modifiers) = living.attribute_modifiers(host, SCALE) else {
        return false;
    };
    match toggle(is_shrunk(&modifiers)) {
        Toggle::Restore => living.remove_attribute_modifier(host, SCALE, modifier_id()),
        Toggle::Shrink => {
            let modifier = AttributeModifier::new(
                modifier_id().clone(),
                total_multiplier_amount(SHRUNK_SCALE),
                AttributeOperation::AddMultipliedTotal,
            );
            living.add_attribute_modifier(host, SCALE, modifier, ModifierLifetime::Permanent)
        }
    }
}

/// The Shrinking Device.
struct Device;

#[mod_api::horizon_part]
impl ItemPart for Device {
    /// Used in the air: shrink or restore the user.
    fn use_item(&self, host: HostSide, event: &mut UseItemEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        let player = host.player(event.player)?;
        let living = host.living(player.entity())?;
        if !use_device(host, living) {
            return None;
        }
        let pitch = pitch_for(&*host, living);
        player.entity().play_sound(host, SOUND, 1.0, pitch);
        Some(InteractionResult::SuccessServer)
    }

    /// Used on a mob: shrink or restore the mob.
    fn item_interact_mob(
        &self,
        host: HostSide,
        event: &mut ItemInteractMobEvent<'_>,
    ) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        let entity = host.entity(u32::try_from(event.entity).ok()?)?;
        let living = host.living(entity)?;
        if !use_device(host, living) {
            return None;
        }
        let pitch = pitch_for(&*host, living);
        entity.play_sound(host, SOUND, 1.0, pitch);
        Some(InteractionResult::SuccessServer)
    }
}

/// The device's sound: Shrink plays a beam-like sound; the port uses the beacon's, higher when
/// the entity has just shrunk.
const SOUND: &str = "minecraft:block.beacon.power_select";

fn pitch_for(host: &dyn mod_api::Host, living: LivingRef) -> f32 {
    match living.attribute_value(host, SCALE) {
        Some(scale) if scale < 0.5 => 2.0,
        _ => 0.8,
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl mod_api::Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let device = r.item_key("shrink:shrinking_device");
        r.item_part(device, Device);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vanilla's formula for a total multiplier: base times one plus the amount.
    fn apply(base: f64, amount: f64) -> f64 {
        base * (1.0 + amount)
    }

    #[test]
    fn the_modifier_brings_a_full_size_entity_to_the_shrunk_scale() {
        assert!((apply(1.0, total_multiplier_amount(SHRUNK_SCALE)) - 0.1).abs() < 1e-12);
        // A baby or an entity already scaled by something else keeps its proportion.
        assert!((apply(0.5, total_multiplier_amount(SHRUNK_SCALE)) - 0.05).abs() < 1e-12);
    }

    #[test]
    fn using_the_device_twice_restores_the_entity() {
        let shrunk = vec![AttributeModifier::new(
            modifier_id().clone(),
            -0.9,
            AttributeOperation::AddMultipliedTotal,
        )];
        assert!(is_shrunk(&shrunk));
        assert_eq!(toggle(is_shrunk(&shrunk)), Toggle::Restore);
        assert_eq!(toggle(is_shrunk(&[])), Toggle::Shrink);
    }

    #[test]
    fn another_mods_scale_modifier_is_not_ours() {
        let other = vec![AttributeModifier::new(
            Identifier::parse("pehkui:base").unwrap(),
            -0.5,
            AttributeOperation::AddMultipliedTotal,
        )];
        assert!(!is_shrunk(&other));
    }
}
