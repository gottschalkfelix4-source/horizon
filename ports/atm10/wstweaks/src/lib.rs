//! Wither Skeleton Tweaks for Horizon: skeletons that join the Nether become wither skeletons
//! (keeping their weapons, so some carry bows), a wither skeleton that drops no skull drops a
//! skull fragment (nine craft a skull), damaged stone swords and bows no longer drop from
//! skeletons, strays and wither skeletons, and the Immolation Blade kills a skeleton-like mob at
//! once and takes its head.
//!
//! Written from the mod's public description, never from its code (docs/modpacks.md,
//! "Licences"). What the port does, and what a harness against the NeoForge original still has
//! to confirm, is in README.md.
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use mod_api::{
    EntityJoinLevelEvent, EquipmentSlot, Flow, Hooks, HurtEnemyEvent, ItemDrop, ItemKey, ItemPart,
    ItemStack, LivingDeathEvent, LivingDropsEvent, Registrar, RegistrationContext, ServerHost,
    ServerTickEvent, SpawnReason,
};

const NETHER: &str = "minecraft:the_nether";
const SKELETON: &str = "minecraft:skeleton";
const WITHER_SKELETON: &str = "minecraft:wither_skeleton";

/// The item keys this mod reads, known once registration ran.
struct Keys {
    fragment: ItemKey,
    blade: ItemKey,
    wither_skull: ItemKey,
    skeleton_skull: ItemKey,
    stone_sword: ItemKey,
    bow: ItemKey,
}

static KEYS: OnceLock<Keys> = OnceLock::new();

/// The skull a beheaded mob of `kind` drops, if it has one.
pub fn head_of(kind: &str) -> Option<Head> {
    match kind {
        WITHER_SKELETON => Some(Head::Wither),
        SKELETON | "minecraft:stray" | "minecraft:bogged" => Some(Head::Skeleton),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Head {
    Wither,
    Skeleton,
}

/// Whether a mob of `kind` that joins `dimension` turns into a wither skeleton. Mobs loaded
/// from disk are left alone: they were checked when they first joined.
pub fn converts(kind: &str, dimension: &str, loaded_from_disk: bool) -> bool {
    kind == SKELETON && dimension == NETHER && !loaded_from_disk
}

/// Whether a mob of `kind` loses its damaged stone swords and bows from its drops.
pub fn cleans_drops(kind: &str) -> bool {
    matches!(kind, SKELETON | "minecraft:stray" | WITHER_SKELETON)
}

/// What the drops become. `damaged_junk` says which drops are a damaged stone sword or bow;
/// `skull` which are a wither skeleton skull. Returns the indices to keep and whether a
/// fragment is added.
pub fn plan_drops(
    kind: &str,
    killed_by_player: bool,
    drops: &[(bool, bool)],
) -> (Vec<usize>, bool) {
    let keep = drops
        .iter()
        .enumerate()
        .filter(|(_, (junk, _))| !(cleans_drops(kind) && *junk))
        .map(|(i, _)| i)
        .collect();
    let has_skull = drops.iter().any(|(_, skull)| *skull);
    let fragment = kind == WITHER_SKELETON && killed_by_player && !has_skull;
    (keep, fragment)
}

/// A skeleton waiting to come back as a wither skeleton at the next server tick.
struct Conversion {
    dimension: String,
    pos: [f64; 3],
    equipment: Vec<(EquipmentSlot, Option<ItemStack>)>,
}

static CONVERSIONS: Mutex<Vec<Conversion>> = Mutex::new(Vec::new());

/// Mobs the Immolation Blade struck, whose drops get their head.
static BEHEADED: Mutex<Option<HashSet<u32>>> = Mutex::new(None);

fn behead(entity: u32) {
    if let Ok(mut set) = BEHEADED.lock() {
        set.get_or_insert_with(HashSet::new).insert(entity);
    }
}

fn take_beheaded(entity: u32) -> bool {
    BEHEADED
        .lock()
        .ok()
        .and_then(|mut set| set.as_mut().map(|s| s.remove(&entity)))
        .unwrap_or(false)
}

const HAND_AND_ARMOR: [EquipmentSlot; 6] = [
    EquipmentSlot::MainHand,
    EquipmentSlot::OffHand,
    EquipmentSlot::Feet,
    EquipmentSlot::Legs,
    EquipmentSlot::Chest,
    EquipmentSlot::Head,
];

/// The Immolation Blade: a hit on a skeleton-like mob kills it and takes its head.
struct ImmolationBlade;

#[mod_api::horizon_part]
impl ItemPart for ImmolationBlade {
    fn hurt_enemy(&self, host: &mut dyn ServerHost, event: &mut HurtEnemyEvent<'_>) {
        if head_of(event.kind).is_none() {
            return;
        }
        let Some(target) = host.entity(event.entity) else {
            return;
        };
        behead(event.entity);
        if target.hurt(host, "minecraft:generic_kill", f32::MAX, None, None) != Some(true) {
            // Already dead (the hit itself killed it, and `living_death` saw the blade) or
            // could not be hurt: nothing more drops.
            take_beheaded(event.entity);
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let keys = Keys {
            fragment: r.item_key("wstweaks:fragment"),
            blade: r.item_key("wstweaks:immolation_blade"),
            wither_skull: r.item_key("minecraft:wither_skeleton_skull"),
            skeleton_skull: r.item_key("minecraft:skeleton_skull"),
            stone_sword: r.item_key("minecraft:stone_sword"),
            bow: r.item_key("minecraft:bow"),
        };
        r.item_part(keys.blade, ImmolationBlade);
        let _ = KEYS.set(keys);
    }

    /// A skeleton joining the Nether is kept out and comes back as a wither skeleton.
    fn entity_join_level(
        &self,
        host: &mut dyn ServerHost,
        event: &mut EntityJoinLevelEvent<'_>,
    ) -> Flow {
        if !converts(event.kind, event.dimension, event.loaded_from_disk) {
            return Flow::Continue;
        }
        let Some(living) = host.entity(event.entity).and_then(|e| host.living(e)) else {
            return Flow::Continue;
        };
        let equipment = HAND_AND_ARMOR
            .into_iter()
            .map(|slot| (slot, living.equipment(host, slot)))
            .collect();
        if let Ok(mut queue) = CONVERSIONS.lock() {
            queue.push(Conversion {
                dimension: event.dimension.to_owned(),
                pos: event.pos,
                equipment,
            });
        }
        Flow::Cancel
    }

    fn server_tick(&self, host: &mut dyn ServerHost, _: &mut ServerTickEvent<'_>) {
        let pending = match CONVERSIONS.lock() {
            Ok(mut queue) if !queue.is_empty() => std::mem::take(&mut *queue),
            _ => return,
        };
        for c in pending {
            let Some(level) = host.level(&c.dimension) else {
                continue;
            };
            let Some(mob) = level.spawn_mob(host, WITHER_SKELETON, c.pos, SpawnReason::Conversion)
            else {
                continue;
            };
            let Some(living) = host.living(mob) else {
                continue;
            };
            for (slot, stack) in c.equipment {
                let _ = living.set_equipment(host, slot, stack);
            }
        }
    }

    /// A mob killed with the blade in hand is beheaded.
    fn living_death(&self, _: &mut dyn ServerHost, event: &mut LivingDeathEvent<'_>) -> Flow {
        let Some(keys) = KEYS.get() else {
            return Flow::Continue;
        };
        if head_of(event.kind).is_some() && event.weapon.is_some_and(|w| w.item() == keys.blade) {
            behead(event.entity);
        }
        Flow::Continue
    }

    fn living_drops(&self, _: &mut dyn ServerHost, event: &mut LivingDropsEvent<'_>) -> Flow {
        let Some(keys) = KEYS.get() else {
            return Flow::Continue;
        };
        let beheaded = take_beheaded(event.entity);
        if !cleans_drops(event.kind) && !beheaded {
            return Flow::Continue;
        }
        let facts: Vec<(bool, bool)> = event
            .drops
            .iter()
            .map(|d| {
                let item = d.stack.item();
                let junk = (item == keys.stone_sword || item == keys.bow) && d.stack.is_damaged();
                (junk, item == keys.wither_skull)
            })
            .collect();
        let (keep, fragment) = plan_drops(event.kind, event.player.is_some(), &facts);
        let at = event.drops.first().map(|d| d.position);
        let old = std::mem::take(event.drops);
        event.drops.extend(
            old.into_iter()
                .enumerate()
                .filter(|(i, _)| keep.contains(i))
                .map(|(_, d)| d),
        );
        let Some(at) = at.or(event.source.position) else {
            return Flow::Continue;
        };
        let drop = |item: ItemKey| ItemDrop::new(ItemStack::of(item), at, [0.0, 0.2, 0.0], 10);
        if beheaded {
            let skull = match head_of(event.kind) {
                Some(Head::Wither) => keys.wither_skull,
                _ => keys.skeleton_skull,
            };
            if !event.drops.iter().any(|d| d.stack.item() == skull) {
                event.drops.push(drop(skull));
            }
        } else if fragment {
            event.drops.push(drop(keys.fragment));
        }
        Flow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_new_skeletons_in_the_nether_convert() {
        assert!(converts(
            "minecraft:skeleton",
            "minecraft:the_nether",
            false
        ));
        assert!(!converts(
            "minecraft:skeleton",
            "minecraft:the_nether",
            true
        ));
        assert!(!converts(
            "minecraft:skeleton",
            "minecraft:overworld",
            false
        ));
        assert!(!converts("minecraft:stray", "minecraft:the_nether", false));
    }

    #[test]
    fn a_skull_less_player_kill_of_a_wither_skeleton_adds_a_fragment() {
        // (damaged junk, wither skull)
        let drops = [(false, false), (true, false)];
        assert_eq!(
            plan_drops("minecraft:wither_skeleton", true, &drops),
            (vec![0], true)
        );
        let with_skull = [(false, true)];
        assert_eq!(
            plan_drops("minecraft:wither_skeleton", true, &with_skull),
            (vec![0], false)
        );
        assert_eq!(
            plan_drops("minecraft:wither_skeleton", false, &[]),
            (vec![], false)
        );
    }

    #[test]
    fn damaged_weapons_drop_only_from_mobs_outside_the_list() {
        let drops = [(true, false), (false, false)];
        assert_eq!(
            plan_drops("minecraft:stray", true, &drops),
            (vec![1], false)
        );
        assert_eq!(
            plan_drops("minecraft:zombie", true, &drops),
            (vec![0, 1], false)
        );
    }

    #[test]
    fn heads_follow_the_skeleton_family() {
        assert_eq!(head_of("minecraft:wither_skeleton"), Some(Head::Wither));
        assert_eq!(head_of("minecraft:bogged"), Some(Head::Skeleton));
        assert_eq!(head_of("minecraft:zombie"), None);
    }
}
