//! Ice and Fire for Horizon, first slice: the three elemental dragonbone swords.
//!
//! Written from Ice and Fire's public description and from how the swords play, never from its
//! code (docs/modpacks.md, "Licences"). Every number below is from memory of the mod and is listed
//! in README.md under "To check against the NeoForge original".
//!
//! - `iceandfire:dragonbone_sword_fire` (flamed): sets the target on fire and knocks it back.
//! - `iceandfire:dragonbone_sword_ice` (iced): freezes the target in place and knocks it back.
//! - `iceandfire:dragonbone_sword_lightning`: strikes the target with lightning and knocks it back.
//!
//! Each also deals extra damage to the dragon of the opposite element (a flamed sword to ice
//! dragons, an iced sword to fire dragons, a lightning sword to both). The hit's effects run in
//! `ItemPart::hurt_enemy` (the player's hit landed); the extra damage is added before armour in
//! `living_incoming_damage`, where the hit's weapon is known.
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use mod_api::{
    ActiveEffect, EffectKey, EntityJoinLevelEvent, Flow, Hooks, HostSide, HurtEnemyEvent, ItemKey,
    ItemPart, LevelRandomHost, LevelTickPostEvent, LivingIncomingDamageEvent, Registrar,
    RegistrationContext, ServerHost,
};

/// A sword's element.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    Fire,
    Ice,
    Lightning,
}

impl Element {
    const ALL: [Element; 3] = [Element::Fire, Element::Ice, Element::Lightning];

    fn sword_id(self) -> &'static str {
        match self {
            Element::Fire => "iceandfire:dragonbone_sword_fire",
            Element::Ice => "iceandfire:dragonbone_sword_ice",
            Element::Lightning => "iceandfire:dragonbone_sword_lightning",
        }
    }

    /// What a landed hit does to its target, besides the sword's own damage.
    pub fn on_hit(self) -> HitEffect {
        match self {
            Element::Fire => HitEffect::Ignite { ticks: FIRE_TICKS },
            Element::Ice => HitEffect::Freeze {
                ticks: FREEZE_TICKS,
            },
            Element::Lightning => HitEffect::Lightning,
        }
    }

    /// The extra damage a hit with this element deals to an entity of type `target`.
    pub fn bonus_damage(self, target: &str) -> f32 {
        let weak = match self {
            Element::Fire => target == ICE_DRAGON,
            Element::Ice => target == FIRE_DRAGON,
            Element::Lightning => target == FIRE_DRAGON || target == ICE_DRAGON,
        };
        if weak { DRAGON_BONUS } else { 0.0 }
    }
}

pub const FIRE_DRAGON: &str = "iceandfire:fire_dragon";
pub const ICE_DRAGON: &str = "iceandfire:ice_dragon";
/// Extra damage against the opposite dragon (from memory).
pub const DRAGON_BONUS: f32 = 8.0;
/// Flamed: five seconds of fire (from memory).
pub const FIRE_TICKS: i32 = 100;
/// Iced: frozen for ten seconds (from memory).
pub const FREEZE_TICKS: i32 = 200;
/// Horizontal knockback every elemental sword adds (from memory: about a knockback I).
pub const KNOCKBACK: f64 = 0.5;

/// A hit's elemental effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitEffect {
    Ignite { ticks: i32 },
    Freeze { ticks: i32 },
    Lightning,
}

/// The push away from `attacker` that a hit gives a target standing at `target`: horizontal,
/// `strength` long, plus vanilla knockback's small lift. None when the two stand on the same
/// vertical line (no direction to push in).
pub fn knockback(attacker: [f64; 3], target: [f64; 3], strength: f64) -> Option<[f64; 3]> {
    let dx = target[0] - attacker[0];
    let dz = target[2] - attacker[2];
    let len = (dx * dx + dz * dz).sqrt();
    if len < 1.0e-4 {
        return None;
    }
    Some([dx / len * strength, 0.1, dz / len * strength])
}

/// Until when a mob hit at `now` for `ticks` stays frozen, when it already was until
/// `existing`: a new hit never shortens a freeze.
pub fn freeze_until(existing: Option<i64>, now: i64, ticks: i32) -> i64 {
    let until = now + i64::from(ticks);
    existing.map_or(until, |e| e.max(until))
}

/// The tag a frozen mob carries, so a freeze cut short by a restart is undone when it loads.
const FROZEN_TAG: &str = "iceandfire.frozen";

/// Frozen mobs by entity id: their dimension and the game time they thaw at.
fn frozen() -> std::sync::MutexGuard<'static, HashMap<u32, (String, i64)>> {
    static FROZEN: OnceLock<Mutex<HashMap<u32, (String, i64)>>> = OnceLock::new();
    FROZEN
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Stop a mob's AI until `ticks` from now. Its physics runs on, so knockback still moves it.
fn freeze_mob(host: &mut dyn ServerHost, target: mod_api::EntityRef, ticks: i32) {
    let Some(mob) = host.mob(target) else {
        return;
    };
    let Some(level) = target.level(host) else {
        return;
    };
    let (Some(dimension), Some(now)) = (level.dimension().id(host), level.game_time(host)) else {
        return;
    };
    if mob.set_ai_paused(host, true).is_err() {
        return;
    }
    target.add_tag(host, FROZEN_TAG);
    let mut frozen = frozen();
    let existing = frozen.get(&target.id()).map(|(_, until)| *until);
    let until = freeze_until(existing, now, ticks);
    frozen.insert(target.id(), (dimension, until));
}

fn thaw(host: &mut dyn ServerHost, target: mod_api::EntityRef) {
    if let Some(mob) = host.mob(target) {
        let _ = mob.set_ai_paused(host, false);
    }
    target.remove_tag(host, FROZEN_TAG);
}

/// The three swords' keys and the effects the hits use, known once registration ran.
struct Keys {
    swords: [ItemKey; 3],
    slowness: EffectKey,
}

static KEYS: OnceLock<Keys> = OnceLock::new();

fn element_of(item: ItemKey) -> Option<Element> {
    let keys = KEYS.get()?;
    Element::ALL
        .into_iter()
        .zip(keys.swords)
        .find_map(|(element, key)| (key == item).then_some(element))
}

/// One elemental sword.
struct Sword(Element);

#[mod_api::horizon_part]
impl ItemPart for Sword {
    /// The player's hit landed: apply the element and the knockback.
    fn hurt_enemy(&self, host: &mut dyn ServerHost, event: &mut HurtEnemyEvent<'_>) {
        let Some(target) = host.entity(event.entity) else {
            return;
        };
        let Some(target_pos) = target.position(host) else {
            return;
        };
        match self.0.on_hit() {
            HitEffect::Ignite { ticks } => {
                if target.fire_immune(host) != Some(true) {
                    let left = target.fire_ticks(host).unwrap_or(0).max(ticks);
                    target.set_remaining_fire_ticks(host, left);
                }
            }
            HitEffect::Freeze { ticks } => {
                // Ice and Fire encases the mob in ice and stops it moving: its AI is paused, and
                // slowness VII (seven levels take off 105 % of the speed) holds a player still.
                freeze_mob(host, target, ticks);
                if let (Some(living), Some(keys)) = (host.living(target), KEYS.get()) {
                    let effect = ActiveEffect::new(keys.slowness, ticks, 6, false, true, true);
                    living.add_effect(host, &effect);
                }
                target.extinguish_fire(host);
            }
            HitEffect::Lightning => {
                let dimension = target
                    .level(host)
                    .and_then(|level| level.dimension().id(host));
                if let Some(dimension) = dimension {
                    host.summon(
                        &dimension,
                        "minecraft:lightning_bolt",
                        target_pos,
                        Default::default(),
                    );
                }
            }
        }
        let attacker = host
            .players()
            .acting_player()
            .and_then(|player| player.entity().position(host));
        if let Some(push) = attacker.and_then(|from| knockback(from, target_pos, KNOCKBACK)) {
            target.push(host, push);
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let swords = Element::ALL.map(|element| r.item_key(element.sword_id()));
        let slowness = r.effect_key("minecraft:slowness");
        let _ = KEYS.set(Keys { swords, slowness });
        for (element, key) in Element::ALL.into_iter().zip(swords) {
            r.item_part(key, Sword(element));
        }
    }

    /// Thaw the mobs of this level whose freeze is over.
    fn level_tick_post(&self, host: HostSide, event: &mut LevelTickPostEvent) {
        let HostSide::Server(host) = host else {
            return;
        };
        if frozen().is_empty() {
            return;
        }
        let (Some(here), Some(now)) = (
            event.level.dimension().id(host),
            event.level.game_time(host),
        ) else {
            return;
        };
        let thawed: Vec<u32> = {
            let mut frozen = frozen();
            let ids: Vec<u32> = frozen
                .iter()
                .filter(|(_, (dim, until))| *dim == here && now >= *until)
                .map(|(id, _)| *id)
                .collect();
            for id in &ids {
                frozen.remove(id);
            }
            ids
        };
        for id in thawed {
            if let Some(target) = host.entity(id) {
                thaw(host, target);
            }
        }
    }

    /// A mob saved while frozen comes back thawed.
    fn entity_join_level(
        &self,
        host: &mut dyn ServerHost,
        event: &mut EntityJoinLevelEvent<'_>,
    ) -> Flow {
        if event.loaded_from_disk
            && let Some(entity) = host.entity(event.entity)
            && entity
                .tags(host)
                .is_some_and(|tags| tags.iter().any(|t| t == FROZEN_TAG))
        {
            thaw(host, entity);
        }
        Flow::Continue
    }

    /// Add the extra damage against the opposite dragon before armour takes its share.
    fn living_incoming_damage(
        &self,
        _: &mut dyn LevelRandomHost,
        event: &mut LivingIncomingDamageEvent<'_>,
    ) -> Flow {
        let Some(element) = event.weapon.and_then(|stack| element_of(stack.item())) else {
            return Flow::Continue;
        };
        let bonus = element.bonus_damage(event.kind);
        if bonus > 0.0 {
            let damage = event.container.new_damage() + bonus;
            event.container.set_new_damage(damage);
        }
        Flow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_sword_is_strong_against_the_other_element() {
        assert_eq!(Element::Fire.bonus_damage(ICE_DRAGON), 8.0);
        assert_eq!(Element::Fire.bonus_damage(FIRE_DRAGON), 0.0);
        assert_eq!(Element::Ice.bonus_damage(FIRE_DRAGON), 8.0);
        assert_eq!(Element::Ice.bonus_damage(ICE_DRAGON), 0.0);
        assert_eq!(Element::Lightning.bonus_damage(FIRE_DRAGON), 8.0);
        assert_eq!(Element::Lightning.bonus_damage(ICE_DRAGON), 8.0);
        assert_eq!(Element::Fire.bonus_damage("minecraft:zombie"), 0.0);
    }

    #[test]
    fn knockback_points_away_from_the_attacker() {
        let push = knockback([0.0, 64.0, 0.0], [3.0, 64.0, 4.0], 0.5).unwrap();
        assert!((push[0] - 0.3).abs() < 1e-9);
        assert!((push[2] - 0.4).abs() < 1e-9);
        assert!(push[1] > 0.0);
        let back = knockback([10.0, 0.0, 10.0], [9.0, 0.0, 10.0], 1.0).unwrap();
        assert!((back[0] + 1.0).abs() < 1e-9 && back[2].abs() < 1e-9);
    }

    #[test]
    fn no_knockback_without_a_horizontal_direction() {
        assert_eq!(knockback([1.0, 0.0, 1.0], [1.0, 5.0, 1.0], 0.5), None);
    }

    #[test]
    fn a_new_hit_extends_a_freeze_but_never_shortens_it() {
        assert_eq!(freeze_until(None, 1000, 200), 1200);
        assert_eq!(freeze_until(Some(1100), 1050, 200), 1250);
        assert_eq!(freeze_until(Some(5000), 1050, 200), 5000);
    }

    #[test]
    fn each_element_has_its_own_hit_effect() {
        assert_eq!(Element::Fire.on_hit(), HitEffect::Ignite { ticks: 100 });
        assert_eq!(Element::Ice.on_hit(), HitEffect::Freeze { ticks: 200 });
        assert_eq!(Element::Lightning.on_hit(), HitEffect::Lightning);
    }
}
