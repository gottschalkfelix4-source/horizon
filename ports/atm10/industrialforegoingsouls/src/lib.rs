//! Industrial Foregoing Souls for Horizon: the Soul Surge.
//!
//! Written from the mod's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). Confidence in this port is low: see README.md for what it is
//! and is not.
//!
//! A Soul Surge is bound to a mob by using that mob's spawn egg on it. While it has no redstone
//! signal and holds [`COST`] energy, every [`PERIOD`] ticks it spawns one of that mob on top of
//! itself, unless [`LIMIT`] or more creatures already stand in the area around it.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Compound, Hooks, Host, InteractionResult, LevelRef, LevelSide,
    ModEnergyBuffer, Registrar, RegistrationContext, ServerHost, SpawnReason, Tag, Text,
    UseItemOnEvent,
};

pub const PERIOD: i32 = 100;
pub const COST: i32 = 5_000;
pub const CAPACITY: i32 = 100_000;
pub const MAX_INSERT: i32 = 5_000;
/// Half the side of the area counted, and its height.
pub const RADIUS: f64 = 4.0;
pub const LIMIT: usize = 8;

/// The mob a spawn egg item stands for: `minecraft:zombie_spawn_egg` is `minecraft:zombie`.
pub fn mob_of_egg(item: &str) -> Option<String> {
    let (ns, path) = item.split_once(':')?;
    let mob = path.strip_suffix("_spawn_egg")?;
    (!mob.is_empty()).then(|| format!("{ns}:{mob}"))
}

/// Whether a surge may spawn: bound, powered by energy, not stopped by redstone, and with room.
pub fn may_spawn(bound: bool, energy: i32, redstone: bool, nearby: usize) -> bool {
    bound && energy >= COST && !redstone && nearby < LIMIT
}

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

fn mob(nbt: &Compound) -> Option<String> {
    match nbt.get(&"mob".into()) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    }
}

fn operate(host: &mut dyn ServerHost, level: LevelRef, event: &mut BlockEntityTickEvent<'_>) -> bool {
    let nbt = &event.entity.nbt;
    let (energy, bound) = (int(nbt, "energy"), mob(nbt));
    let [x, y, z] = event.pos.map(f64::from);
    let nearby = level
        .entities_in(&*host, [x - RADIUS, y - 1.0, z - RADIUS], [x + RADIUS + 1.0, y + 3.0, z + RADIUS + 1.0])
        .map_or(0, |e| e.len());
    let redstone = level.has_neighbor_signal(&*host, event.pos);
    if !may_spawn(bound.is_some(), energy, redstone, nearby) {
        return false;
    }
    let Some(kind) = bound else { return false };
    if level.spawn_mob(host, &kind, [x + 0.5, y + 1.0, z + 0.5], SpawnReason::Spawner).is_none() {
        return false;
    }
    event.entity.nbt.insert("energy".into(), Tag::Int(energy - COST));
    true
}

struct Surge;

#[mod_api::horizon_part]
impl BlockPart for Surge {
    fn use_item_on(&self, host: &mut dyn ServerHost, event: &mut UseItemOnEvent<'_>) -> Option<InteractionResult> {
        let click = event.click;
        let level = host.level(host.dimension())?;
        if level.block(&*host, click.pos)?.id(&*host) != "industrialforegoingsouls:soul_surge" {
            return None;
        }
        let kind = mob_of_egg(&click.stack?.item().id(&*host))?;
        let entity = host.block_entity(level, click.pos)?;
        let mut patch = Compound::new();
        patch.insert("mob".into(), Tag::String(kind.as_str().into()));
        let _ = entity.merge_data(host, patch);
        if let Some(p) = host.player(click.player) {
            let _ = p.action_bar(host, Text::translate("industrialforegoingsouls.bound", vec![Text::literal(&kind)]));
        }
        Some(InteractionResult::SuccessServer)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for Surge {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == "industrialforegoingsouls:soul_surge").then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == "industrialforegoingsouls:soul_surge" {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let phase = (int(&event.entity.nbt, "phase") + 1) % PERIOD;
        event.entity.nbt.insert("phase".into(), Tag::Int(phase));
        if phase != 0 {
            return;
        }
        let Some(level) = host.level(host.dimension()) else { return };
        if !operate(host, level, event) {
            *event.reply = BlockEntityTickReply::Unchanged;
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let key = r.block_key("industrialforegoingsouls:soul_surge");
        r.block_part(key, Surge);
        r.block_energy(&key, ModEnergyBuffer {
            capacity: CAPACITY,
            max_insert: MAX_INSERT,
            max_extract: 0,
            insert_faces: [true; 6],
            extract_faces: [false; 6],
        });
        let kind = r
            .block_entity_type("soul_surge")
            .blocks(&["industrialforegoingsouls:soul_surge"])
            .build();
        let _ = r.block_entity_part(&kind, Surge);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spawn_egg_names_its_mob_and_other_items_none() {
        assert_eq!(mob_of_egg("minecraft:zombie_spawn_egg").as_deref(), Some("minecraft:zombie"));
        assert_eq!(mob_of_egg("othermod:slime_spawn_egg").as_deref(), Some("othermod:slime"));
        assert_eq!(mob_of_egg("minecraft:diamond"), None);
        assert_eq!(mob_of_egg("minecraft:_spawn_egg"), None);
    }

    #[test]
    fn a_surge_spawns_only_when_bound_charged_unstopped_and_not_crowded() {
        assert!(may_spawn(true, COST, false, LIMIT - 1));
        assert!(!may_spawn(false, COST, false, 0));
        assert!(!may_spawn(true, COST - 1, false, 0));
        assert!(!may_spawn(true, COST, true, 0));
        assert!(!may_spawn(true, COST, false, LIMIT));
    }
}
