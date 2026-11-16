//! Apothic Spawners for Horizon: right-click a monster spawner with a modifier item to change its
//! delays, spawn count, nearby limit, ranges and behaviour; Quartz in the off hand inverts the
//! change. A spawner broken with Silk Touch drops itself, keeping its mob and numbers.
//!
//! Written from Apothic Spawners' public description, never from its code (docs/modpacks.md,
//! "Licences"). What the port does, and what a harness against the NeoForge original still has
//! to confirm, is in README.md.
//!
//! How: the six numbers are vanilla's own spawner fields, written into the block entity's data.
//! The switches vanilla has no field for (no AI, silent, burning...) live in an attachment on the
//! spawner's block entity. A mob the spawner makes is tagged in `finalize_spawn` with the switches
//! that concern it, and `entity_join_level` applies them once it exists.

pub mod modifiers;

use std::sync::OnceLock;

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentHolder, AttachmentQueries, AttachmentType,
    BlockDropsEvent, BlockKey, BlockPart, Compound, EntityJoinLevelEvent, FinalizeSpawnEvent, Flow,
    Hand, Holder, Hooks, Host, InteractionResult, ItemDrop, ItemKey, ItemStack, LevelHost,
    LivingDropsEvent, MobSpawnPlacementCheckEvent, MobSpawnPositionCheckEvent, Registrar,
    RegistrationContext, ServerHost, SpawnCheck, SpawnReason, SpawnSource,
    Spawner as SpawnerSource, Tag, UseItemOnEvent,
};

use modifiers::{Extras, Flag, INVERTER, MODIFIERS, Modifier, Spawner, Stats, apply};

/// The switches and health change kept beside one spawner: `{flags: S, health: I}`.
static EXTRAS: AttachmentType<Extras> = AttachmentType {
    namespace: "apothic_spawners",
    name: "extras",
    default: Extras::default,
    codec: Some(AttachmentCodec {
        encode: encode_extras,
        decode: decode_extras,
    }),
    copy_on_death: false,
    sync: false,
};

fn encode_extras(extras: &Extras) -> Result<Compound, AttachmentError> {
    Ok([
        ("flags".into(), Tag::Int(i32::from(extras.flags))),
        ("health".into(), Tag::Int(extras.health_percent)),
    ]
    .into())
}

fn decode_extras(data: &Compound) -> Result<Extras, AttachmentError> {
    let int = |key: &str| match data.get(&key.into()) {
        Some(Tag::Int(v)) => Ok(*v),
        None => Ok(0),
        _ => Err(AttachmentError::Decode),
    };
    Ok(Extras {
        flags: u16::try_from(int("flags")?).map_err(|_| AttachmentError::Decode)?,
        health_percent: int("health")?,
    })
}

/// The tag a spawned mob carries for its initial health change, `apothic_spawners.health=<pct>`.
const HEALTH_TAG: &str = "apothic_spawners.health=";

/// The switches applied to the mob itself once it joins the level.
const MOB_FLAGS: [Flag; 5] = [
    Flag::Echoing,
    Flag::Burning,
    Flag::Youthful,
    Flag::Silent,
    Flag::NoAi,
];

/// How long a Burning spawner's mob burns, in ticks (assumed: "spawns on fire", README).
const BURN_TICKS: i32 = i16::MAX as i32;

/// The tags a spawner with `extras` gives its mobs.
pub fn mob_tags(extras: Extras) -> Vec<String> {
    let mut tags: Vec<String> = MOB_FLAGS
        .iter()
        .filter(|f| extras.has(**f))
        .map(|f| f.mob_tag().to_string())
        .collect();
    if extras.health_percent != 0 {
        tags.push(format!("{HEALTH_TAG}{}", extras.health_percent));
    }
    tags
}

/// The health a mob starts with, `percent` away from its max health (never below half a heart).
pub fn initial_health(max_health: f32, percent: i32) -> f32 {
    (max_health * (100 + percent) as f32 / 100.0).max(0.5)
}

/// Whether an `minecraft:enchantments` component (26.3's id-to-level map, or the older
/// `{levels: {...}}` form) carries Silk Touch.
pub fn has_silk_touch(enchantments: &Tag) -> bool {
    let Tag::Compound(map) = enchantments else {
        return false;
    };
    let map = match map.get(&"levels".into()) {
        Some(Tag::Compound(levels)) => levels,
        _ => map,
    };
    matches!(map.get(&"minecraft:silk_touch".into()), Some(Tag::Int(l)) if *l > 0)
}

/// The vanilla spawner's block and item.
static KEYS: OnceLock<(BlockKey, ItemKey)> = OnceLock::new();

/// Each modifier's item key, in `MODIFIERS` order, and the inverter's.
static ITEMS: OnceLock<(Vec<ItemKey>, ItemKey)> = OnceLock::new();

fn modifier_of(item: ItemKey) -> Option<&'static Modifier> {
    let (keys, _) = ITEMS.get()?;
    keys.iter().position(|k| *k == item).map(|i| &MODIFIERS[i])
}

fn is_inverter(item: ItemKey) -> bool {
    ITEMS.get().is_some_and(|(_, quartz)| *quartz == item)
}

/// A spawner's extras, read where the hook has only a `Host`.
fn extras_at(host: &dyn Host, dimension: &str, pos: [i32; 3]) -> Extras {
    let Some(level) = host.level(dimension) else {
        return Extras::default();
    };
    let Some(entity) = host.block_entity(level, pos) else {
        return Extras::default();
    };
    host.query(&EXTRAS)
        .ok()
        .and_then(|q| q.get(AttachmentHolder::BlockEntity(entity)).ok().flatten())
        .unwrap_or_default()
}

/// The modifier items on `minecraft:spawner`.
struct SpawnerPart;

#[mod_api::horizon_part]
impl BlockPart for SpawnerPart {
    fn use_item_on(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseItemOnEvent<'_>,
    ) -> Option<InteractionResult> {
        let click = event.click;
        let stack = click.stack?;
        let modifier = modifier_of(stack.item())?;
        let level = host.level(click.dimension.key().id())?;
        let player = host.player(click.player)?;
        let living = host.living(player.entity())?;
        let inverted = living
            .hand_item(host, Hand::Off)
            .is_some_and(|off| is_inverter(off.item()));
        let entity = host.block_entity(level, click.pos)?;
        let stats = Stats::from_nbt(&entity.data(host)?);
        let holder = Holder::BlockEntity(click.pos);
        let extras = EXTRAS.get(host.attachments(), holder).ok()?;
        let before = Spawner { stats, extras };
        let Some(after) = apply(before, modifier.change, inverted) else {
            // At its cap: the item is not used, and the click goes on as vanilla's would.
            return None;
        };
        if after.stats != before.stats {
            entity.merge_data(host, after.stats.to_nbt()).ok()?;
        }
        if after.extras != before.extras {
            EXTRAS.set(host.attachments(), holder, after.extras).ok()?;
        }
        if !player.is_creative(host).unwrap_or(false) {
            let _ = living.set_hand_item(host, click.hand, stack.clone().shrink(1));
        }
        Some(InteractionResult::Success)
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let block = r.block_key("minecraft:spawner");
        let item = r.item_key("minecraft:spawner");
        let _ = KEYS.set((block, item));
        let modifiers = MODIFIERS.iter().map(|m| r.item_key(m.item)).collect();
        let _ = ITEMS.set((modifiers, r.item_key(INVERTER)));
        r.block_part(block, SpawnerPart);
    }

    /// Tag a spawner's mob with the switches that concern it.
    fn finalize_spawn(&self, host: &dyn Host, event: &mut FinalizeSpawnEvent<'_>) -> Flow {
        let (SpawnReason::Spawner, Some(SpawnSource::Block(pos))) = (event.reason, event.spawner)
        else {
            return Flow::Continue;
        };
        let extras = extras_at(host, event.dimension, [pos.x, pos.y, pos.z]);
        for tag in mob_tags(extras) {
            let _ = event.edits.add_tag(event.mob, &tag);
        }
        Flow::Continue
    }

    /// Apply the tagged switches once the mob exists.
    fn entity_join_level(
        &self,
        host: &mut dyn ServerHost,
        event: &mut EntityJoinLevelEvent<'_>,
    ) -> Flow {
        if event.loaded_from_disk {
            return Flow::Continue;
        }
        let Some(entity) = host.entity(event.entity) else {
            return Flow::Continue;
        };
        let Some(tags) = entity.tags(host) else {
            return Flow::Continue;
        };
        for tag in tags.iter().filter(|t| t.starts_with("apothic_spawners.")) {
            if let Some(percent) = tag.strip_prefix(HEALTH_TAG) {
                if let (Ok(percent), Some(living)) = (percent.parse(), host.living(entity))
                    && let Some(max) = living.max_health(host)
                {
                    living.set_health(host, initial_health(max, percent));
                }
                entity.remove_tag(host, tag);
                continue;
            }
            let Some(flag) = MOB_FLAGS.iter().find(|f| f.mob_tag() == tag) else {
                continue;
            };
            match flag {
                // Kept: the drops read it when the mob dies.
                Flag::Echoing => continue,
                Flag::Burning => {
                    entity.set_remaining_fire_ticks(host, BURN_TICKS);
                }
                Flag::Silent => {
                    entity.set_silent(host, true);
                }
                Flag::NoAi => {
                    if let Some(mob) = host.mob(entity) {
                        mob.set_no_ai(host, true);
                    }
                }
                Flag::Youthful => {
                    // `Age` for animals and villagers, `IsBaby` for zombies and piglins.
                    let baby: Compound = [
                        ("Age".into(), Tag::Int(-24000)),
                        ("IsBaby".into(), Tag::Byte(1)),
                    ]
                    .into();
                    entity.merge_nbt(host, baby);
                }
                _ => {}
            }
            entity.remove_tag(host, tag);
        }
        Flow::Continue
    }

    /// Echoing: a mob from such a spawner drops its loot twice.
    fn living_drops(&self, host: &mut dyn ServerHost, event: &mut LivingDropsEvent<'_>) -> Flow {
        let echoing = host
            .entity(event.entity)
            .and_then(|e| e.tags(host))
            .is_some_and(|tags| tags.iter().any(|t| t == Flag::Echoing.mob_tag()));
        if echoing {
            let copies: Vec<ItemDrop> = event.drops.clone();
            event.drops.extend(copies);
        }
        Flow::Continue
    }

    /// Ignore Conditions: a spawner's mob may spawn where its placement rules would refuse it.
    fn mob_spawn_placement_check(
        &self,
        host: &dyn Host,
        event: &mut MobSpawnPlacementCheckEvent<'_>,
    ) {
        if let Some(SpawnerSource::Block { pos }) = event.spawner
            && *event.result == SpawnCheck::Default
            && extras_at(host, event.dimension, [pos.x, pos.y, pos.z]).has(Flag::IgnoreConditions)
        {
            *event.result = SpawnCheck::Succeed;
        }
    }

    /// Ignore Conditions also passes the mob's own position check (`Mob.checkSpawnRules`).
    fn mob_spawn_position_check(
        &self,
        host: &dyn Host,
        event: &mut MobSpawnPositionCheckEvent<'_>,
    ) {
        if let Some(SpawnerSource::Block { pos }) = event.spawner
            && *event.result == SpawnCheck::Default
            && extras_at(host, event.dimension, [pos.x, pos.y, pos.z]).has(Flag::IgnoreConditions)
        {
            *event.result = SpawnCheck::Succeed;
        }
    }

    /// Silk Touch picks the spawner up with its mob and numbers.
    fn block_drops(&self, level: &dyn LevelHost, event: &mut BlockDropsEvent<'_>) -> Flow {
        let Some((block, item)) = KEYS.get() else {
            return Flow::Continue;
        };
        if event.state.block() != *block || event.breaker.is_none() {
            return Flow::Continue;
        }
        let silk = event
            .tool
            .and_then(ItemStack::components)
            .and_then(|c| c.as_nbt().get(&"minecraft:enchantments".into()))
            .is_some_and(has_silk_touch);
        if !silk {
            return Flow::Continue;
        }
        let Some(mut data) = event.block_entity.and_then(|e| level.block_entity_data(e)) else {
            return Flow::Continue;
        };
        data.insert("id".into(), Tag::String("minecraft:mob_spawner".into()));
        let mut stack = ItemStack::of(*item);
        let patch: Compound = [("minecraft:block_entity_data".into(), Tag::Compound(data))].into();
        if stack.apply_components(&patch).is_err() {
            return Flow::Continue;
        }
        let at = [
            f64::from(event.pos.x) + 0.5,
            f64::from(event.pos.y) + 0.5,
            f64::from(event.pos.z) + 0.5,
        ];
        event.drops.push(ItemDrop::new(stack, at, [0.0; 3], 10));
        event.experience = 0;
        Flow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extras_round_trip_through_their_saved_form() {
        let extras = Extras {
            flags: Flag::Silent.bit() | Flag::NoAi.bit(),
            health_percent: -15,
        };
        assert_eq!(
            decode_extras(&encode_extras(&extras).unwrap()).unwrap(),
            extras
        );
    }

    #[test]
    fn a_spawner_tags_its_mobs_only_with_the_switches_that_concern_them() {
        let extras = Extras {
            flags: Flag::IgnorePlayers.bit() | Flag::Burning.bit(),
            health_percent: -10,
        };
        assert_eq!(
            mob_tags(extras),
            vec![
                "apothic_spawners.burning".to_string(),
                "apothic_spawners.health=-10".to_string()
            ]
        );
    }

    #[test]
    fn initial_health_is_a_share_of_max_health() {
        assert_eq!(initial_health(20.0, -25), 15.0);
        assert_eq!(initial_health(1.0, -95), 0.5);
    }

    #[test]
    fn silk_touch_is_found_in_both_component_forms() {
        let level = |l: i32| -> Compound { [("minecraft:silk_touch".into(), Tag::Int(l))].into() };
        assert!(has_silk_touch(&Tag::Compound(level(1))));
        let wrapped: Compound = [("levels".into(), Tag::Compound(level(1)))].into();
        assert!(has_silk_touch(&Tag::Compound(wrapped)));
        let other: Compound = [("minecraft:fortune".into(), Tag::Int(3))].into();
        assert!(!has_silk_touch(&Tag::Compound(other)));
    }
}
