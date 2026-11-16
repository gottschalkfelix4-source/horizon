//! SecurityCraft for Horizon, first slice: reinforced blocks. A reinforced block belongs to the
//! player who placed it; only that player can break it, and explosions cannot (its explosion
//! resistance, a block fact in `data/`, is bedrock's).
//!
//! Written from SecurityCraft's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and what a harness against the NeoForge
//! original still has to confirm, is in README.md.
//!
//! How it works: placing a reinforced block from its item records the placer as its owner on the
//! chunk it stands in (a chunk attachment, saved with the chunk). When a player breaks a block,
//! `break_block` looks the position up: a reinforced block with a recorded owner other than the
//! breaker is kept (the break is cancelled and the breaker is told who owns it). The owner's break
//! goes ahead and forgets the record.
use std::sync::OnceLock;

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockKey, BreakBlockEvent, Compound, Flow,
    Holder, Hooks, ItemPart, OnBlockPlacedByEvent, Registrar, RegistrationContext, ServerHost, Tag,
    Text,
};

/// The reinforced blocks this slice registers. Each keeps its vanilla counterpart's look and
/// hardness (`data/securitycraft/horizon/block/*.json`).
pub const REINFORCED: [&str; 4] = [
    "securitycraft:reinforced_stone",
    "securitycraft:reinforced_cobblestone",
    "securitycraft:reinforced_oak_planks",
    "securitycraft:reinforced_glass",
];

/// One owned block: where it is, and who placed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owned {
    pub pos: [i32; 3],
    pub owner: u128,
    /// The owner's name when they placed it, for the message a non-owner gets.
    pub name: String,
}

/// What a break of a reinforced block comes to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Nobody owns it (placed by a dispenser, `/setblock`, a structure): anyone breaks it.
    Unowned,
    /// The owner breaks it: it goes, and so does the record.
    Owner,
    /// Someone else: the block stays. Carries the owner's name.
    Refused(String),
}

/// Who may break the block at `pos`, as the records of its chunk say. `breaker` is None when
/// no player is acting (the hook only fires for a player, so that is only a guard).
pub fn verdict(records: &[Owned], pos: [i32; 3], breaker: Option<u128>) -> Verdict {
    let Some(record) = records.iter().find(|r| r.pos == pos) else {
        return Verdict::Unowned;
    };
    match breaker {
        Some(uuid) if uuid == record.owner => Verdict::Owner,
        _ => Verdict::Refused(record.name.clone()),
    }
}

/// Record `owned` on its chunk's list, replacing a record at the same position.
pub fn record(records: &mut Vec<Owned>, owned: Owned) {
    records.retain(|r| r.pos != owned.pos);
    records.push(owned);
}

/// The owned blocks recorded on one chunk, saved as
/// `{blocks: [{pos: [I; x, y, z], owner: [I; 4 words], name: "..."}]}`.
static OWNERS: AttachmentType<Vec<Owned>> = AttachmentType {
    namespace: "securitycraft",
    name: "owners",
    default: Vec::new,
    codec: Some(AttachmentCodec {
        encode: encode_owners,
        decode: decode_owners,
    }),
    copy_on_death: false,
    sync: false,
};

fn uuid_words(uuid: u128) -> Vec<i32> {
    (0..4)
        .rev()
        .map(|i| (uuid >> (32 * i)) as u32 as i32)
        .collect()
}

fn words_uuid(words: &[i32]) -> u128 {
    words
        .iter()
        .fold(0u128, |acc, w| (acc << 32) | u128::from(*w as u32))
}

fn encode_owners(records: &Vec<Owned>) -> Result<Compound, AttachmentError> {
    let list = records
        .iter()
        .map(|r| {
            Tag::Compound(
                [
                    ("pos".into(), Tag::IntArray(r.pos.to_vec())),
                    ("owner".into(), Tag::IntArray(uuid_words(r.owner))),
                    ("name".into(), Tag::String(r.name.as_str().into())),
                ]
                .into(),
            )
        })
        .collect();
    Ok([("blocks".into(), Tag::List(list))].into())
}

fn decode_owners(data: &Compound) -> Result<Vec<Owned>, AttachmentError> {
    let Some(Tag::List(list)) = data.get(&"blocks".into()) else {
        return Err(AttachmentError::Decode);
    };
    list.iter()
        .map(|tag| {
            let Tag::Compound(c) = tag else {
                return Err(AttachmentError::Decode);
            };
            let (Some(Tag::IntArray(pos)), Some(Tag::IntArray(owner))) =
                (c.get(&"pos".into()), c.get(&"owner".into()))
            else {
                return Err(AttachmentError::Decode);
            };
            if pos.len() != 3 || owner.len() != 4 {
                return Err(AttachmentError::Decode);
            }
            let name = match c.get(&"name".into()) {
                Some(Tag::String(s)) => s.to_string_lossy(),
                _ => String::new(),
            };
            Ok(Owned {
                pos: [pos[0], pos[1], pos[2]],
                owner: words_uuid(owner),
                name,
            })
        })
        .collect()
}

/// The reinforced blocks' keys, known once registration ran.
static KEYS: OnceLock<Vec<BlockKey>> = OnceLock::new();

fn is_reinforced(block: BlockKey) -> bool {
    KEYS.get().is_some_and(|keys| keys.contains(&block))
}

fn chunk_of(pos: [i32; 3]) -> Holder {
    Holder::Chunk([pos[0] >> 4, pos[2] >> 4])
}

/// The reinforced blocks' items: placing one records its placer as the owner.
struct ReinforcedItem;

#[mod_api::horizon_part]
impl ItemPart for ReinforcedItem {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        if !is_reinforced(event.state.block()) {
            return;
        }
        // A dispenser has no placer: the block stays unowned, as SecurityCraft leaves it.
        let Some(placer) = event.placer else {
            return;
        };
        let name = placer.name(host).unwrap_or_default();
        let chunk = chunk_of(event.pos);
        let Ok(mut records) = OWNERS.get(host.attachments(), chunk) else {
            return;
        };
        record(
            &mut records,
            Owned {
                pos: event.pos,
                owner: placer.uuid(),
                name,
            },
        );
        let _ = OWNERS.set(host.attachments(), chunk, records);
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let keys = REINFORCED.iter().map(|id| r.block_key(id)).collect();
        let _ = KEYS.set(keys);
        for id in REINFORCED {
            let item = r.item_key(id);
            r.item_part(item, ReinforcedItem);
        }
    }

    /// Keep a reinforced block a player other than its owner breaks.
    fn break_block(&self, host: &mut dyn ServerHost, event: &mut BreakBlockEvent<'_>) -> Flow {
        if event.cancelled || !is_reinforced(event.state.block()) {
            return Flow::Continue;
        }
        let chunk = chunk_of(event.pos);
        let Ok(mut records) = OWNERS.get(host.attachments(), chunk) else {
            return Flow::Continue;
        };
        let player = host.players().acting_player();
        match verdict(&records, event.pos, player.map(|p| p.uuid())) {
            Verdict::Unowned => Flow::Continue,
            Verdict::Owner => {
                records.retain(|r| r.pos != event.pos);
                let _ = OWNERS.set(host.attachments(), chunk, records);
                Flow::Continue
            }
            Verdict::Refused(owner) => {
                event.cancelled = true;
                if let Some(player) = player {
                    let line = Text::translate(
                        "securitycraft.message.owned_by",
                        vec![Text::literal(owner)],
                    );
                    let _ = player.tell(host, line);
                }
                Flow::Cancel
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: u128 = 0x0123_4567_89ab_cdef_0011_2233_4455_6677;
    const BOB: u128 = 0xffee_ddcc_bbaa_9988_7766_5544_3322_1100;

    fn alice_at(pos: [i32; 3]) -> Owned {
        Owned {
            pos,
            owner: ALICE,
            name: "Alice".into(),
        }
    }

    #[test]
    fn only_the_owner_breaks_an_owned_block() {
        let records = vec![alice_at([4, 70, -3])];
        assert_eq!(verdict(&records, [4, 70, -3], Some(ALICE)), Verdict::Owner);
        assert_eq!(
            verdict(&records, [4, 70, -3], Some(BOB)),
            Verdict::Refused("Alice".into())
        );
        assert_eq!(
            verdict(&records, [4, 70, -3], None),
            Verdict::Refused("Alice".into())
        );
    }

    #[test]
    fn a_block_without_a_record_is_anyones() {
        let records = vec![alice_at([4, 70, -3])];
        assert_eq!(verdict(&records, [4, 71, -3], Some(BOB)), Verdict::Unowned);
    }

    #[test]
    fn placing_again_at_a_position_replaces_its_owner() {
        let mut records = vec![alice_at([0, 64, 0]), alice_at([1, 64, 0])];
        record(
            &mut records,
            Owned {
                pos: [0, 64, 0],
                owner: BOB,
                name: "Bob".into(),
            },
        );
        assert_eq!(records.len(), 2);
        assert_eq!(verdict(&records, [0, 64, 0], Some(BOB)), Verdict::Owner);
        assert_eq!(verdict(&records, [1, 64, 0], Some(ALICE)), Verdict::Owner);
    }

    #[test]
    fn owners_round_trip_through_their_saved_form() {
        let records = vec![
            alice_at([-17, -60, 300]),
            Owned {
                pos: [5, 200, 9],
                owner: BOB,
                name: "Bob".into(),
            },
        ];
        let saved = encode_owners(&records).unwrap();
        assert_eq!(decode_owners(&saved).unwrap(), records);
    }
}
