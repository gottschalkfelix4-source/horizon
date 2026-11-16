//! Neo Vitae for Horizon: the tier 1 Blood Altar, filled by the Sacrificial Dagger, and its
//! infusion crafting.
//!
//! Written from the public description of Blood Magic / Neo Vitae (wiki, guide book, how the
//! mod plays), never from its code. What still has to be confirmed against the NeoForge
//! original is listed in README.md.
//!
//! How it works:
//! - Right-clicking with `neovitae:sacrificial_dagger` costs the player one heart and pours
//!   [`LP_PER_HEART`] life essence into a Blood Altar within [`DAGGER_REACH`] blocks.
//! - Right-clicking an altar with a recipe input (stone, a diamond) puts one item on it; an empty
//!   hand takes what lies on it back.
//! - Every server tick each loaded altar works on its item: it drinks `consume` LP a tick into
//!   the recipe's progress until the recipe's LP is paid, and the item turns into the result.
//!   Out of LP, progress falls back by `drain` a tick.
//!
//! Altars are recorded on their chunk (attachment `neovitae:altars`, saved with the chunk) when
//! placed from their item; a record whose block is gone is dropped on the next tick.
use std::sync::OnceLock;

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentQueriesMut, AttachmentType, BlockKey, Compound,
    Holder, HostSide, HostTransaction, InteractionResult, ItemKey, ItemPart, ItemPlace,
    ItemResource, ItemStack, LevelRef, OnBlockPlacedByEvent, Registrar, RegistrationContext,
    ServerHost, Tag, UseItemEvent, UseItemOnEvent, UseWithoutItemEvent,
};

/// Life essence one heart (2 health) gives through the dagger, before runes.
pub const LP_PER_HEART: i32 = 200;
/// Health the dagger takes.
pub const DAGGER_HEALTH_COST: f32 = 2.0;
/// How far from the player (a cube, blocks on each axis) the dagger looks for an altar.
pub const DAGGER_REACH: i32 = 2;
/// A tier 1 altar's tank, in LP, with no capacity runes.
pub const TIER1_CAPACITY: i32 = 10_000;

/// One altar infusion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AltarRecipe {
    pub input: &'static str,
    pub output: &'static str,
    /// The lowest altar tier that crafts it.
    pub tier: u8,
    /// LP the whole infusion costs.
    pub lp: i32,
    /// LP drunk per tick while it works.
    pub consume: i32,
    /// Progress lost per tick while the altar is dry.
    pub drain: i32,
}

/// The tier 1 infusions this port knows (from memory: check against the original's recipes).
pub const RECIPES: [AltarRecipe; 2] = [
    AltarRecipe {
        input: "minecraft:stone",
        output: "neovitae:blank_slate",
        tier: 1,
        lp: 1000,
        consume: 5,
        drain: 5,
    },
    AltarRecipe {
        input: "minecraft:diamond",
        output: "neovitae:weak_blood_orb",
        tier: 1,
        lp: 2000,
        consume: 2,
        drain: 1,
    },
];

/// What lies on an altar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Empty,
    /// The input of `RECIPES[recipe]`, with the LP paid into it so far.
    Working {
        recipe: usize,
        progress: i32,
    },
    /// The finished result of `RECIPES[recipe]`.
    Done {
        recipe: usize,
    },
}

/// One placed Blood Altar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Altar {
    pub pos: [i32; 3],
    pub lp: i32,
    pub slot: Slot,
}

/// What one tick did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickOutcome {
    Idle,
    Worked,
    Finished,
}

impl Altar {
    pub fn new(pos: [i32; 3]) -> Self {
        Self {
            pos,
            lp: 0,
            slot: Slot::Empty,
        }
    }

    /// Pour `amount` LP in, up to the tank; returns what went in.
    pub fn fill(&mut self, amount: i32) -> i32 {
        let added = amount.min(TIER1_CAPACITY - self.lp).max(0);
        self.lp += added;
        added
    }

    /// One tick of infusion (tier 1, no speed runes).
    pub fn tick(&mut self) -> TickOutcome {
        let Slot::Working { recipe, progress } = self.slot else {
            return TickOutcome::Idle;
        };
        let r = RECIPES[recipe];
        if self.lp >= r.consume {
            let step = r.consume.min(r.lp - progress);
            self.lp -= step;
            let progress = progress + step;
            if progress >= r.lp {
                self.slot = Slot::Done { recipe };
                return TickOutcome::Finished;
            }
            self.slot = Slot::Working { recipe, progress };
            TickOutcome::Worked
        } else if progress > 0 {
            self.slot = Slot::Working {
                recipe,
                progress: (progress - r.drain).max(0),
            };
            TickOutcome::Worked
        } else {
            TickOutcome::Idle
        }
    }

    /// Whether `pos` is within dagger reach of this altar.
    pub fn in_reach(&self, pos: [i32; 3]) -> bool {
        (0..3).all(|i| (pos[i] - self.pos[i]).abs() <= DAGGER_REACH)
    }
}

/// The altars recorded on one chunk, saved as `{altars: [I; x, y, z, lp, slot, progress, ...]}`
/// with `slot` -1 for empty, `2 * recipe` working and `2 * recipe + 1` done.
static ALTARS: AttachmentType<Vec<Altar>> = AttachmentType {
    namespace: "neovitae",
    name: "altars",
    default: Vec::new,
    codec: Some(AttachmentCodec {
        encode: encode_altars,
        decode: decode_altars,
    }),
    copy_on_death: false,
    sync: false,
};

fn encode_altars(altars: &Vec<Altar>) -> Result<Compound, AttachmentError> {
    let flat = altars
        .iter()
        .flat_map(|a| {
            let (slot, progress) = match a.slot {
                Slot::Empty => (-1, 0),
                Slot::Working { recipe, progress } => (2 * recipe as i32, progress),
                Slot::Done { recipe } => (2 * recipe as i32 + 1, 0),
            };
            [a.pos[0], a.pos[1], a.pos[2], a.lp, slot, progress]
        })
        .collect();
    Ok([("altars".into(), Tag::IntArray(flat))].into())
}

fn decode_altars(data: &Compound) -> Result<Vec<Altar>, AttachmentError> {
    let Some(Tag::IntArray(flat)) = data.get(&"altars".into()) else {
        return Err(AttachmentError::Decode);
    };
    if flat.len() % 6 != 0 {
        return Err(AttachmentError::Decode);
    }
    Ok(flat
        .chunks_exact(6)
        .map(|c| {
            let recipe = (c[4] / 2) as usize;
            let slot = match c[4] {
                s if s < 0 || recipe >= RECIPES.len() => Slot::Empty,
                s if s % 2 == 0 => Slot::Working {
                    recipe,
                    progress: c[5],
                },
                _ => Slot::Done { recipe },
            };
            Altar {
                pos: [c[0], c[1], c[2]],
                lp: c[3].clamp(0, TIER1_CAPACITY),
                slot,
            }
        })
        .collect())
}

/// Keys resolved at registration.
struct Keys {
    altar: BlockKey,
    inputs: Vec<ItemKey>,
    outputs: Vec<ItemKey>,
}

static KEYS: OnceLock<Keys> = OnceLock::new();

fn chunk_of(pos: [i32; 3]) -> Holder {
    Holder::Chunk([pos[0] >> 4, pos[2] >> 4])
}

fn is_altar(host: &dyn ServerHost, level: LevelRef, pos: [i32; 3]) -> bool {
    KEYS.get()
        .is_some_and(|k| level.block(host, pos) == Some(k.altar))
}

/// Read, change and write back the altar record at `pos`.
fn with_altar<R>(
    host: &mut dyn ServerHost,
    pos: [i32; 3],
    f: impl FnOnce(&mut Altar) -> R,
) -> Option<R> {
    let holder = chunk_of(pos);
    let mut altars = ALTARS.get(host.attachments(), holder).ok()?;
    let altar = match altars.iter_mut().position(|a| a.pos == pos) {
        Some(i) => &mut altars[i],
        // An altar the record missed (placed by a machine, `/setblock`): start it empty.
        None => {
            altars.push(Altar::new(pos));
            altars.last_mut()?
        }
    };
    let out = f(altar);
    ALTARS.set(host.attachments(), holder, altars).ok()?;
    Some(out)
}

/// The altar's item: placing it records the altar.
struct AltarItem;

#[mod_api::horizon_part]
impl ItemPart for AltarItem {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        if KEYS.get().is_none_or(|k| event.state.block() != k.altar) {
            return;
        }
        let pos = event.pos;
        let holder = chunk_of(pos);
        let Ok(mut altars) = ALTARS.get(host.attachments(), holder) else {
            return;
        };
        altars.retain(|a| a.pos != pos);
        altars.push(Altar::new(pos));
        let _ = ALTARS.set(host.attachments(), holder, altars);
    }
}

/// The Blood Altar block: takes an input from the hand, gives its item back to an empty hand.
struct AltarBlock;

#[mod_api::horizon_part]
impl mod_api::BlockPart for AltarBlock {
    fn use_item_on(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseItemOnEvent<'_>,
    ) -> Option<InteractionResult> {
        let keys = KEYS.get()?;
        let click = event.click;
        let stack = click.stack?;
        let recipe = keys.inputs.iter().position(|k| *k == stack.item())?;
        if RECIPES[recipe].tier > 1 {
            return None;
        }
        let pos = click.pos;
        let empty = with_altar(host, pos, |a| a.slot == Slot::Empty)?;
        if !empty {
            return Some(InteractionResult::Pass);
        }
        let place = ItemPlace::PlayerHand {
            player: click.player,
            hand: click.hand,
        };
        let resource = ItemResource::of(stack);
        let taken: Result<u32, ()> =
            host.transaction(|host, tx| Ok(place.extract(host, tx, &resource, 1)));
        if taken != Ok(1) {
            return Some(InteractionResult::Fail);
        }
        with_altar(host, pos, |a| {
            a.slot = Slot::Working {
                recipe,
                progress: 0,
            }
        });
        Some(InteractionResult::SuccessServer)
    }

    fn use_without_item(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseWithoutItemEvent<'_>,
    ) -> Option<InteractionResult> {
        let keys = KEYS.get()?;
        let click = event.click;
        let pos = click.pos;
        let taken = with_altar(host, pos, |a| std::mem::replace(&mut a.slot, Slot::Empty))?;
        let item = match taken {
            Slot::Empty => return None,
            Slot::Working { recipe, .. } => keys.inputs[recipe],
            Slot::Done { recipe } => keys.outputs[recipe],
        };
        // Into the (empty) hand; what doesn't fit goes where vanilla puts it: the inventory.
        let place = ItemPlace::PlayerHand {
            player: click.player,
            hand: click.hand,
        };
        let resource = ItemResource::of(&ItemStack::of(item));
        let given: Result<u32, ()> =
            host.transaction(|host, tx| Ok(place.insert(host, tx, &resource, 1)));
        if given != Ok(1) {
            let player = host.player(click.player)?;
            let level = player.level(host)?;
            let at = player.entity().position(host)?;
            let _ = level.spawn_item(host, at[0], at[1], at[2], ItemStack::of(item), 0);
        }
        Some(InteractionResult::SuccessServer)
    }
}

/// The Sacrificial Dagger: one heart for [`LP_PER_HEART`] in a nearby altar.
struct Dagger;

#[mod_api::horizon_part]
impl ItemPart for Dagger {
    fn use_item(&self, host: HostSide, event: &mut UseItemEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        let player = host.player(event.player)?;
        let level = player.level(host)?;
        let feet = player.entity().block_position(host)?;
        if player.has_infinite_materials(host) != Some(true) {
            player
                .entity()
                .hurt(host, "minecraft:magic", DAGGER_HEALTH_COST, None, None);
        }
        fill_nearest_altar(host, level, feet, LP_PER_HEART);
        Some(InteractionResult::SuccessServer)
    }
}

/// Pour `amount` into the first altar in reach of `pos` that still stands.
fn fill_nearest_altar(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], amount: i32) {
    let Ok(query) = host.query_mut(&ALTARS) else {
        return;
    };
    let mut target = None;
    for row in query.iter().filter_map(Result::ok) {
        if let Some(altar) = row
            .value()
            .iter()
            .find(|a| a.in_reach(pos) && is_altar(host, level, a.pos))
        {
            target = Some(altar.pos);
            break;
        }
    }
    drop(query);
    if let Some(altar) = target {
        with_altar(host, altar, |a| a.fill(amount));
    }
}

/// Tick every loaded altar of `level`.
fn tick_altars(host: &mut dyn ServerHost, level: LevelRef) {
    let Ok(query) = host.query_mut(&ALTARS) else {
        return;
    };
    let rows: Vec<_> = query.iter().filter_map(Result::ok).collect();
    for row in rows {
        let mut altars = row.value().clone();
        let before = altars.clone();
        altars.retain(|a| is_altar(host, level, a.pos));
        let mut finished = Vec::new();
        for altar in &mut altars {
            if altar.tick() == TickOutcome::Finished {
                finished.push(altar.pos);
            }
        }
        if altars != before {
            let _ = row.set(host, altars);
        }
        for pos in finished {
            let at = [
                pos[0] as f64 + 0.5,
                pos[1] as f64 + 1.0,
                pos[2] as f64 + 0.5,
            ];
            level.play_sound(
                host,
                "minecraft:block.brewing_stand.brew",
                at,
                mod_api::SoundSource::Blocks,
                0.5,
                1.0,
            );
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl mod_api::Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let altar = r.block_key("neovitae:blood_altar");
        let keys = Keys {
            altar,
            inputs: RECIPES.iter().map(|x| r.item_key(x.input)).collect(),
            outputs: RECIPES.iter().map(|x| r.item_key(x.output)).collect(),
        };
        let _ = KEYS.set(keys);
        let altar_item = r.item_key("neovitae:blood_altar");
        r.item_part(altar_item, AltarItem);
        r.block_part(altar, AltarBlock);
        let dagger = r.item_key("neovitae:sacrificial_dagger");
        r.item_part(dagger, Dagger);
    }

    fn level_tick_post(&self, host: HostSide, event: &mut mod_api::LevelTickPostEvent) {
        if let HostSide::Server(host) = host {
            tick_altars(host, event.level);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dagger_fills_the_tank_up_to_ten_thousand() {
        let mut altar = Altar::new([0, 64, 0]);
        let mut poured = 0;
        for _ in 0..60 {
            poured += altar.fill(LP_PER_HEART);
        }
        assert_eq!(poured, 10_000);
        assert_eq!(altar.lp, 10_000);
        assert_eq!(altar.fill(LP_PER_HEART), 0);
    }

    #[test]
    fn a_blank_slate_takes_two_hundred_ticks_and_a_thousand_lp() {
        let mut altar = Altar::new([0, 64, 0]);
        altar.fill(1500);
        altar.slot = Slot::Working {
            recipe: 0,
            progress: 0,
        };
        let mut ticks = 0;
        while altar.tick() != TickOutcome::Finished {
            ticks += 1;
            assert!(ticks < 1000);
        }
        assert_eq!(ticks + 1, 200);
        assert_eq!(altar.lp, 500);
        assert_eq!(altar.slot, Slot::Done { recipe: 0 });
    }

    #[test]
    fn a_dry_altar_loses_progress_and_stops_at_zero() {
        let mut altar = Altar::new([0, 64, 0]);
        altar.slot = Slot::Working {
            recipe: 0,
            progress: 12,
        };
        altar.tick();
        assert_eq!(
            altar.slot,
            Slot::Working {
                recipe: 0,
                progress: 7
            }
        );
        altar.tick();
        altar.tick();
        assert_eq!(
            altar.slot,
            Slot::Working {
                recipe: 0,
                progress: 0
            }
        );
        assert_eq!(altar.tick(), TickOutcome::Idle);
    }

    #[test]
    fn altars_round_trip_through_their_saved_form() {
        let altars = vec![
            Altar {
                pos: [3, -10, 7],
                lp: 4200,
                slot: Slot::Working {
                    recipe: 1,
                    progress: 300,
                },
            },
            Altar {
                pos: [-1, 70, 2],
                lp: 0,
                slot: Slot::Done { recipe: 0 },
            },
            Altar::new([5, 5, 5]),
        ];
        let saved = encode_altars(&altars).unwrap();
        assert_eq!(decode_altars(&saved).unwrap(), altars);
    }
}
