//! The teleporter: whoever stands on the block (in the two blocks above it) is sent to another
//! teleporter with the same frequency, for energy. Stand-in: the frequency is the item in the
//! block's first slot (hopper menu), not a name in a screen; no portal frame is needed; both
//! must be in the same dimension; no portable teleporter. A teleporter finds its partners
//! through a list of the ones that ticked lately, kept in memory and checked on use.
use std::collections::HashMap;
use std::sync::Mutex;

use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Compound, Host, LevelRef, LevelSide, ModContainer,
    ModEnergyBuffer, Registrar, ServerHost, Tag,
};

pub const BLOCK: &str = "mekanism:teleporter";
pub const CAPACITY: i32 = 100_000;
pub const BASE_COST: i32 = 400;
pub const COST_PER_BLOCK: i32 = 8;
/// Ticks both ends rest after a trip, so nobody is sent back at once.
pub const REST: i32 = 60;

/// FE for a trip between two teleporters.
pub fn trip_cost(from: [i32; 3], to: [i32; 3]) -> i32 {
    let d: f64 = (0..3).map(|i| f64::from(from[i] - to[i]).powi(2)).sum::<f64>().sqrt();
    BASE_COST + (d.round() as i32).saturating_mul(COST_PER_BLOCK)
}

/// Frequency (dimension and item) to the positions that ticked with it.
static KNOWN: Mutex<Option<HashMap<String, Vec<[i32; 3]>>>> = Mutex::new(None);

fn remember(key: &str, pos: [i32; 3]) {
    let mut guard = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
    let list = guard.get_or_insert_with(HashMap::new).entry(key.to_owned()).or_default();
    if !list.contains(&pos) {
        list.push(pos);
    }
}

fn forget(key: &str, pos: [i32; 3]) {
    let mut guard = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(list) = guard.get_or_insert_with(HashMap::new).get_mut(key) {
        list.retain(|p| *p != pos);
    }
}

fn candidates(key: &str, except: [i32; 3]) -> Vec<[i32; 3]> {
    let guard = KNOWN.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()
        .and_then(|m| m.get(key))
        .map(|l| l.iter().copied().filter(|p| *p != except).collect())
        .unwrap_or_default()
}

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

fn text(nbt: &Compound, key: &str) -> Option<String> {
    match nbt.get(&key.into()) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()).filter(|s| !s.is_empty()),
        _ => None,
    }
}

/// A partner that is still a teleporter with this frequency and is rested: its position.
fn partner(host: &dyn Host, level: LevelRef, key: &str, pos: [i32; 3]) -> Option<[i32; 3]> {
    for p in candidates(key, pos) {
        let alive = level.block(host, p).is_some_and(|b| b.id(host) == BLOCK);
        if !alive {
            forget(key, p);
            continue;
        }
        let data = host.block_entity(level, p).and_then(|e| e.data(host));
        if data.is_some_and(|d| text(&d, "freq").as_deref() == key.split_once('|').map(|k| k.1) && int(&d, "rest") == 0) {
            return Some(p);
        }
    }
    None
}

fn tick(host: &mut dyn ServerHost, level: LevelRef, event: &mut BlockEntityTickEvent<'_>) -> bool {
    let pos = event.pos;
    let mut nbt = event.entity.nbt.clone();
    let freq = event
        .entity
        .items
        .iter()
        .flatten()
        .next()
        .map(|s| s.item().id(&*host));
    match &freq {
        Some(f) => nbt.insert("freq".into(), Tag::String(f.as_str().into())),
        None => nbt.remove(&"freq".into()),
    };
    let rest = int(&nbt, "rest");
    if rest > 0 {
        nbt.insert("rest".into(), Tag::Int(rest - 1));
    }
    if let Some(f) = &freq {
        let key = format!("{}|{f}", host.dimension());
        remember(&key, pos);
        let mut energy = int(&nbt, "energy");
        // only whoever stepped onto the block since the last tick travels: staying put on it
        // (or arriving on it) does not send anyone back
        let at = [f64::from(pos[0]), f64::from(pos[1]) + 1.0, f64::from(pos[2])];
        let present = level
            .entities_in(&*host, at, [at[0] + 1.0, at[1] + 2.0, at[2] + 1.0])
            .unwrap_or_default();
        let on: Vec<i32> = match nbt.get(&"on".into()) {
            Some(Tag::IntArray(v)) => v.clone(),
            _ => Vec::new(),
        };
        let mut keep: Vec<i32> = present.iter().map(|e| e.id() as i32).filter(|i| on.contains(i)).collect();
        let newcomers: Vec<_> = present.iter().copied().filter(|e| !on.contains(&(e.id() as i32))).collect();
        if rest == 0 && !newcomers.is_empty() {
            if let Some(to) = partner(&*host, level, &key, pos) {
                let cost = trip_cost(pos, to);
                if energy >= cost {
                    let mut moved = Vec::new();
                    for e in newcomers {
                        let id = e.id() as i32;
                        if e.teleport_to(
                            host,
                            [f64::from(to[0]) + 0.5, f64::from(to[1]) + 1.0, f64::from(to[2]) + 0.5],
                            None,
                        ) {
                            moved.push(id);
                        }
                    }
                    if !moved.is_empty() {
                        energy -= cost;
                        nbt.insert("rest".into(), Tag::Int(REST));
                        if let Some(other) = host.block_entity(level, to) {
                            let mut patch = Compound::new();
                            patch.insert("rest".into(), Tag::Int(REST));
                            let mut all = match other.data(&*host).and_then(|d| d.get(&"on".into()).cloned()) {
                                Some(Tag::IntArray(v)) => v,
                                _ => Vec::new(),
                            };
                            all.extend(moved);
                            patch.insert("on".into(), Tag::IntArray(all));
                            let _ = other.merge_data(host, patch);
                        }
                    }
                }
            }
        }
        keep.sort_unstable();
        nbt.insert("on".into(), Tag::IntArray(keep));
        nbt.insert("energy".into(), Tag::Int(energy));
    }
    if nbt == event.entity.nbt {
        return false;
    }
    event.entity.nbt = nbt;
    true
}

struct Teleporter;

#[mod_api::horizon_part]
impl BlockPart for Teleporter {}

#[mod_api::horizon_part]
impl BlockEntityPart for Teleporter {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == BLOCK).then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == BLOCK {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let Some(level) = host.level(host.dimension()) else { return };
        if !tick(host, level, event) {
            *event.reply = BlockEntityTickReply::Unchanged;
        }
    }
}

pub fn register(r: &mut dyn Registrar) {
    let key = r.block_key(BLOCK);
    r.block_energy(
        &key,
        ModEnergyBuffer {
            capacity: CAPACITY,
            max_insert: 10_000,
            max_extract: 0,
            insert_faces: [true; 6],
            extract_faces: [false; 6],
        },
    );
    r.block_container(
        &key,
        ModContainer {
            slots: 5,
            faces: std::array::from_fn(|_| vec![]),
            inputs: vec![],
            outputs: vec![],
            menu: Some("minecraft:hopper".into()),
            slot_limit: 0,
        },
    );
    let built = r.block_entity_type("teleporter").blocks(&[BLOCK]).build();
    let _ = r.block_entity_part(&built, Teleporter);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trip_costs_a_base_and_the_distance() {
        assert_eq!(trip_cost([0, 64, 0], [0, 64, 0]), 400);
        assert_eq!(trip_cost([0, 64, 0], [30, 64, 40]), 400 + 50 * 8);
    }

    #[test]
    fn partners_come_from_the_list_without_the_asker() {
        remember("t|a", [1, 2, 3]);
        remember("t|a", [4, 5, 6]);
        assert_eq!(candidates("t|a", [1, 2, 3]), vec![[4, 5, 6]]);
        forget("t|a", [4, 5, 6]);
        assert!(candidates("t|a", [1, 2, 3]).is_empty());
    }
}
