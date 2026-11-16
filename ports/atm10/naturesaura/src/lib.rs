//! Nature's Aura for Horizon: the Aura of the world, kept per chunk as drain spots, the Flower
//! Generator that turns flowers into Aura, and the Environmental Eye that reads it.
//!
//! Written from Nature's Aura's public description (the Book of Natural Aura, the wiki, how the
//! mod plays), never from its code. What still has to be confirmed against the NeoForge original
//! is listed in README.md.
//!
//! How it works:
//! - Every place in the world starts with [`DEFAULT_AURA`]. Devices that take or give Aura move
//!   it at a *spot*: a position with a signed excess (positive: more Aura than default,
//!   negative: drained). Spots are kept on their chunk (attachment `naturesaura:spots`, saved).
//!   The Aura around a position is the default plus every spot within [`AREA_RADIUS`].
//! - Storing Aura adds to the lowest spot in range (or starts one at the device); draining
//!   takes from the highest.
//! - `naturesaura:flower_generator`, placed from its item, eats one small flower within
//!   [`FLOWER_REACH`] every [`GENERATOR_INTERVAL`] ticks and stores Aura for it. The same flower
//!   again and again gives less: the yield falls with how many of its last [`MEMORY`] flowers were
//!   of that kind.
//! - `naturesaura:eye` (right click) tells the Aura around the player, in percent of default.
use std::sync::OnceLock;

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentHolder, AttachmentQueries, AttachmentType,
    BlockKey, Compound, Holder, Host, HostSide, InteractionResult, ItemPart, LevelRef,
    OnBlockPlacedByEvent, Registrar, RegistrationContext, ServerHost, Tag, Text, UseItemEvent,
};

/// The Aura every place has before anything drains or stores.
pub const DEFAULT_AURA: i64 = 1_000_000;
/// How far around a position its Aura is counted and spots are looked for, in blocks.
pub const AREA_RADIUS: i32 = 20;
/// How far the Flower Generator reaches for flowers (cube, each axis; 1 up and down).
pub const FLOWER_REACH: i32 = 3;
/// Ticks between two flowers.
pub const GENERATOR_INTERVAL: i64 = 10;
/// Aura one flower of a kind not eaten lately gives.
pub const FLOWER_AURA: i32 = 1000;
/// How many recent flowers the generator remembers.
pub const MEMORY: usize = 10;

/// One drain spot: a position and its signed excess of Aura.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spot {
    pub pos: [i32; 3],
    pub amount: i32,
}

fn within(a: [i32; 3], b: [i32; 3], r: i32) -> bool {
    let d: i64 = (0..3).map(|i| ((a[i] - b[i]) as i64).pow(2)).sum();
    d <= (r as i64) * (r as i64)
}

/// The Aura at `pos` given every spot nearby.
pub fn aura_at<'a>(pos: [i32; 3], spots: impl IntoIterator<Item = &'a Spot>) -> i64 {
    DEFAULT_AURA
        + spots
            .into_iter()
            .filter(|s| within(s.pos, pos, AREA_RADIUS))
            .map(|s| s.amount as i64)
            .sum::<i64>()
}

/// Which spot a device at `pos` moves Aura at: storing (`store` > 0) uses the lowest spot in
/// range, draining the highest; with none in range, a new spot at `pos`.
pub fn spot_for<'a>(
    pos: [i32; 3],
    spots: impl IntoIterator<Item = &'a Spot>,
    store: bool,
) -> [i32; 3] {
    let near = spots
        .into_iter()
        .filter(|s| within(s.pos, pos, AREA_RADIUS));
    let pick = if store {
        near.min_by_key(|s| (s.amount, s.pos))
    } else {
        near.max_by_key(|s| (s.amount, std::cmp::Reverse(s.pos)))
    };
    pick.map_or(pos, |s| s.pos)
}

/// Add `delta` at `pos` in one chunk's spots; a spot back at zero goes away.
pub fn add_at(spots: &mut Vec<Spot>, pos: [i32; 3], delta: i32) {
    match spots.iter().position(|s| s.pos == pos) {
        Some(i) => {
            spots[i].amount = spots[i].amount.saturating_add(delta);
            if spots[i].amount == 0 {
                spots.remove(i);
            }
        }
        None if delta != 0 => spots.push(Spot { pos, amount: delta }),
        None => {}
    }
}

/// The Aura a flower gives when `same` of the generator's last [`MEMORY`] flowers were its kind.
pub fn flower_yield(same: usize) -> i32 {
    let left = MEMORY.saturating_sub(same) as i32;
    FLOWER_AURA * left / MEMORY as i32
}

/// Remember `flower` as eaten, forgetting the oldest past [`MEMORY`].
pub fn remember(recent: &mut Vec<String>, flower: &str) {
    recent.push(flower.to_owned());
    if recent.len() > MEMORY {
        recent.remove(0);
    }
}

static SPOTS: AttachmentType<Vec<Spot>> = AttachmentType {
    namespace: "naturesaura",
    name: "spots",
    default: Vec::new,
    codec: Some(AttachmentCodec {
        encode: encode_spots,
        decode: decode_spots,
    }),
    copy_on_death: false,
    sync: false,
};

fn encode_spots(spots: &Vec<Spot>) -> Result<Compound, AttachmentError> {
    let flat = spots
        .iter()
        .flat_map(|s| [s.pos[0], s.pos[1], s.pos[2], s.amount])
        .collect();
    Ok([("spots".into(), Tag::IntArray(flat))].into())
}

fn decode_spots(data: &Compound) -> Result<Vec<Spot>, AttachmentError> {
    let Some(Tag::IntArray(flat)) = data.get(&"spots".into()) else {
        return Err(AttachmentError::Decode);
    };
    if flat.len() % 4 != 0 {
        return Err(AttachmentError::Decode);
    }
    Ok(flat
        .chunks_exact(4)
        .map(|c| Spot {
            pos: [c[0], c[1], c[2]],
            amount: c[3],
        })
        .collect())
}

/// One Flower Generator and the flowers it ate lately.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Generator {
    pub pos: [i32; 3],
    pub recent: Vec<String>,
}

static GENERATORS: AttachmentType<Vec<Generator>> = AttachmentType {
    namespace: "naturesaura",
    name: "flower_generators",
    default: Vec::new,
    codec: Some(AttachmentCodec {
        encode: encode_generators,
        decode: decode_generators,
    }),
    copy_on_death: false,
    sync: false,
};

fn encode_generators(gens: &Vec<Generator>) -> Result<Compound, AttachmentError> {
    let list = gens
        .iter()
        .map(|g| {
            let recent = g
                .recent
                .iter()
                .map(|f| Tag::String(f.as_str().into()))
                .collect();
            Tag::Compound(
                [
                    ("pos".into(), Tag::IntArray(g.pos.to_vec())),
                    ("recent".into(), Tag::List(recent)),
                ]
                .into(),
            )
        })
        .collect();
    Ok([("generators".into(), Tag::List(list))].into())
}

fn decode_generators(data: &Compound) -> Result<Vec<Generator>, AttachmentError> {
    let Some(Tag::List(list)) = data.get(&"generators".into()) else {
        return Err(AttachmentError::Decode);
    };
    list.iter()
        .map(|entry| {
            let Tag::Compound(c) = entry else {
                return Err(AttachmentError::Decode);
            };
            let Some(Tag::IntArray(pos)) = c.get(&"pos".into()) else {
                return Err(AttachmentError::Decode);
            };
            let [x, y, z] = pos[..] else {
                return Err(AttachmentError::Decode);
            };
            let recent = match c.get(&"recent".into()) {
                Some(Tag::List(items)) => items
                    .iter()
                    .filter_map(|t| match t {
                        Tag::String(s) => s.to_string().ok(),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            Ok(Generator {
                pos: [x, y, z],
                recent,
            })
        })
        .collect()
}

fn chunk_of(pos: [i32; 3]) -> Holder {
    Holder::Chunk([pos[0] >> 4, pos[2] >> 4])
}

/// Every spot of the loaded chunks near `pos`.
fn spots_near(host: &dyn Host, pos: [i32; 3]) -> Vec<Spot> {
    let Ok(query) = host.query(&SPOTS) else {
        return Vec::new();
    };
    let reach = (AREA_RADIUS >> 4) + 1;
    query
        .iter()
        .filter_map(Result::ok)
        .filter(|row| match row.holder {
            AttachmentHolder::Chunk(chunk) => {
                let [cx, cz] = chunk.pos();
                (cx - (pos[0] >> 4)).abs() <= reach && (cz - (pos[2] >> 4)).abs() <= reach
            }
            _ => false,
        })
        .flat_map(|row| row.value.clone())
        .collect()
}

/// Store (`delta` > 0) or drain (`delta` < 0) Aura for a device at `pos`.
fn move_aura(host: &mut dyn ServerHost, pos: [i32; 3], delta: i32) {
    let near = spots_near(host, pos);
    let at = spot_for(pos, &near, delta > 0);
    let holder = chunk_of(at);
    if let Ok(mut spots) = SPOTS.get(host.attachments(), holder) {
        add_at(&mut spots, at, delta);
        let _ = SPOTS.set(host.attachments(), holder, spots);
    }
}

struct Keys {
    generator: BlockKey,
}

static KEYS: OnceLock<Keys> = OnceLock::new();

fn block_id(host: &dyn Host, block: BlockKey) -> Option<String> {
    let id = host.block_state_string(host.block_state_default(block)?)?;
    Some(id.split('[').next().unwrap_or(&id).to_owned())
}

/// One generator's turn: eat a flower nearby and store its Aura. Returns the changed record.
fn run_generator(
    host: &mut dyn ServerHost,
    level: LevelRef,
    generator: &Generator,
    turn: i64,
) -> Option<Generator> {
    let r = FLOWER_REACH;
    let mut flowers = Vec::new();
    for dx in -r..=r {
        for dy in -1..=1 {
            for dz in -r..=r {
                let p = [
                    generator.pos[0] + dx,
                    generator.pos[1] + dy,
                    generator.pos[2] + dz,
                ];
                if let Some(b) = level.block(host, p) {
                    if host.block_in_tag(b, "minecraft:small_flowers") == Some(true) {
                        flowers.push((p, b));
                    }
                }
            }
        }
    }
    if flowers.is_empty() {
        return None;
    }
    let (pos, block) = flowers[turn.rem_euclid(flowers.len() as i64) as usize];
    let id = block_id(host, block)?;
    if !level.remove_block(host, pos, false) {
        return None;
    }
    let same = generator.recent.iter().filter(|f| **f == id).count();
    let amount = flower_yield(same);
    if amount > 0 {
        move_aura(host, generator.pos, amount);
    }
    let mut next = generator.clone();
    remember(&mut next.recent, &id);
    Some(next)
}

fn tick_generators(host: &mut dyn ServerHost, level: LevelRef) {
    let Some(time) = level.game_time(host) else {
        return;
    };
    if time % GENERATOR_INTERVAL != 0 {
        return;
    }
    let Some(keys) = KEYS.get() else {
        return;
    };
    let Ok(query) = mod_api::AttachmentQueriesMut::query_mut(host, &GENERATORS) else {
        return;
    };
    let rows: Vec<_> = query.iter().filter_map(Result::ok).collect();
    for row in rows {
        let mut gens = row.value().clone();
        let before = gens.clone();
        gens.retain(|g| level.block(host, g.pos) == Some(keys.generator));
        for g in &mut gens {
            if let Some(next) = run_generator(host, level, g, time / GENERATOR_INTERVAL) {
                *g = next;
            }
        }
        if gens != before {
            let _ = row.set(host, gens);
        }
    }
}

/// The generator's item records the generator where it is placed.
struct GeneratorItem;

#[mod_api::horizon_part]
impl ItemPart for GeneratorItem {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        if KEYS
            .get()
            .is_none_or(|k| event.state.block() != k.generator)
        {
            return;
        }
        let pos = event.pos;
        let holder = chunk_of(pos);
        let Ok(mut gens) = GENERATORS.get(host.attachments(), holder) else {
            return;
        };
        gens.retain(|g| g.pos != pos);
        gens.push(Generator {
            pos,
            recent: Vec::new(),
        });
        let _ = GENERATORS.set(host.attachments(), holder, gens);
    }
}

/// The Environmental Eye: right click tells the Aura here.
struct Eye;

#[mod_api::horizon_part]
impl ItemPart for Eye {
    fn use_item(&self, host: HostSide, event: &mut UseItemEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        let player = host.player(event.player)?;
        let pos = player.entity().block_position(host)?;
        let aura = aura_at(pos, &spots_near(host, pos));
        let percent = aura * 100 / DEFAULT_AURA;
        let _ = player.action_bar(
            host,
            Text::translate(
                "message.naturesaura.eye.aura",
                vec![
                    Text::literal(aura.to_string()),
                    Text::literal(percent.to_string()),
                ],
            ),
        );
        Some(InteractionResult::SuccessServer)
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl mod_api::Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let generator = r.block_key("naturesaura:flower_generator");
        let _ = KEYS.set(Keys { generator });
        let item = r.item_key("naturesaura:flower_generator");
        r.item_part(item, GeneratorItem);
        let eye = r.item_key("naturesaura:eye");
        r.item_part(eye, Eye);
    }

    fn level_tick_post(&self, host: HostSide, event: &mut mod_api::LevelTickPostEvent) {
        if let HostSide::Server(host) = host {
            tick_generators(host, event.level);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aura_counts_only_spots_within_the_area() {
        let spots = [
            Spot {
                pos: [0, 64, 0],
                amount: 5000,
            },
            Spot {
                pos: [12, 64, 16],
                amount: -2000,
            },
            Spot {
                pos: [40, 64, 0],
                amount: 900_000,
            },
        ];
        // 12² + 16² = 400 = 20²: on the edge, counted.
        assert_eq!(aura_at([0, 64, 0], &spots), 1_003_000);
    }

    #[test]
    fn storing_fills_the_lowest_spot_and_draining_takes_from_the_highest() {
        let spots = [
            Spot {
                pos: [5, 64, 0],
                amount: -300,
            },
            Spot {
                pos: [-5, 64, 0],
                amount: 800,
            },
        ];
        assert_eq!(spot_for([0, 64, 0], &spots, true), [5, 64, 0]);
        assert_eq!(spot_for([0, 64, 0], &spots, false), [-5, 64, 0]);
        assert_eq!(spot_for([100, 64, 0], &spots, true), [100, 64, 0]);
        let mut chunk = spots.to_vec();
        add_at(&mut chunk, [5, 64, 0], 300);
        assert_eq!(
            chunk,
            vec![Spot {
                pos: [-5, 64, 0],
                amount: 800
            }]
        );
    }

    #[test]
    fn the_same_flower_gives_less_until_variety_returns() {
        let mut recent = Vec::new();
        let mut yields = Vec::new();
        for _ in 0..3 {
            let same = recent
                .iter()
                .filter(|f: &&String| *f == "minecraft:poppy")
                .count();
            yields.push(flower_yield(same));
            remember(&mut recent, "minecraft:poppy");
        }
        assert_eq!(yields, vec![1000, 900, 800]);
        for _ in 0..MEMORY {
            remember(&mut recent, "minecraft:dandelion");
        }
        let same = recent.iter().filter(|f| *f == "minecraft:poppy").count();
        assert_eq!(flower_yield(same), 1000);
    }

    #[test]
    fn generators_round_trip_through_their_saved_form() {
        let gens = vec![Generator {
            pos: [1, 2, 3],
            recent: vec!["minecraft:poppy".into(), "minecraft:allium".into()],
        }];
        let saved = encode_generators(&gens).unwrap();
        assert_eq!(decode_generators(&saved).unwrap(), gens);
    }
}
