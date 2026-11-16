//! Transfer Labels for Horizon: labels stuck on the faces of containers move items between them.
//!
//! Written from Transfer Labels' public description, never from its code (docs/modpacks.md,
//! "Licences"). The mechanics below are the port's reading of that description; README.md lists
//! what a harness against the NeoForge original still has to confirm.
//!
//! Using an extract label or an insert label on a face of a block with item storage sticks the
//! label there: the record (position, face, kind, channel) is kept on the chunk (attachment
//! `transfer_labels:labels`, saved with it). The channel is the label's custom name (renamed in an
//! anvil); unnamed labels share the empty channel. Every [`PERIOD`] ticks each extract label
//! sends up to [`ITEMS_PER_TRANSFER`] items through its face to the insert labels of its channel
//! within [`RANGE`] blocks, starting with a different insert label each time (round robin).
//! Sneak-using a label on a labelled face takes the label off.
use std::sync::atomic::{AtomicU64, Ordering};

use mod_api::{
    move_resources, AttachmentCodec, AttachmentError, AttachmentQueries, AttachmentType, Compound, Direction, Holder,
    HostSide, HostTransaction, Hooks, InteractionResult, ItemKey, ItemPart, ItemStack,
    LevelTickPostEvent, LivingRef, Registrar, RegistrationContext, Tag, UseOnEvent, ITEMS,
};

/// Ticks between two transfers.
pub const PERIOD: i64 = 20;
/// Items one extract label moves per transfer.
pub const ITEMS_PER_TRANSFER: u32 = 16;
/// How far (blocks, on each axis) an insert label may be from the extract label.
pub const RANGE: i32 = 32;

/// What a label does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Extract,
    Insert,
}

/// One label on one face.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub pos: [i32; 3],
    /// The face it is on, by vanilla's direction number (down, up, north, south, west, east).
    pub face: u8,
    pub kind: Kind,
    pub channel: String,
}

impl Label {
    fn within_range(&self, other: &Label) -> bool {
        (0..3).all(|i| (self.pos[i] - other.pos[i]).abs() <= RANGE)
    }
}

/// For each extract label, the insert labels it sends to this round, in order: those of its
/// channel within range, not on its own block, starting `round` places further along the list
/// each round so every insert label gets its turn first.
pub fn plan(labels: &[Label], round: u64) -> Vec<(usize, Vec<usize>)> {
    labels
        .iter()
        .enumerate()
        .filter(|(_, l)| l.kind == Kind::Extract)
        .map(|(i, from)| {
            let targets: Vec<usize> = labels
                .iter()
                .enumerate()
                .filter(|(_, to)| {
                    to.kind == Kind::Insert
                        && to.channel == from.channel
                        && to.pos != from.pos
                        && from.within_range(to)
                })
                .map(|(j, _)| j)
                .collect();
            let mut targets = targets;
            if !targets.is_empty() {
                let shift = (round % targets.len() as u64) as usize;
                targets.rotate_left(shift);
            }
            (i, targets)
        })
        .collect()
}

pub const DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

fn face_of(normal: [i32; 3]) -> Option<u8> {
    Some(match normal {
        [0, -1, 0] => 0,
        [0, 1, 0] => 1,
        [0, 0, -1] => 2,
        [0, 0, 1] => 3,
        [-1, 0, 0] => 4,
        [1, 0, 0] => 5,
        _ => return None,
    })
}

/// The labels on one chunk, saved as `{labels: [{pos: [I;x,y,z], face, kind, channel}, ...]}`.
static LABELS: AttachmentType<Vec<Label>> = AttachmentType {
    namespace: "transfer_labels",
    name: "labels",
    default: Vec::new,
    codec: Some(AttachmentCodec {
        encode: encode_labels,
        decode: decode_labels,
    }),
    copy_on_death: false,
    sync: false,
};

fn encode_labels(labels: &Vec<Label>) -> Result<Compound, AttachmentError> {
    let list = labels
        .iter()
        .map(|l| {
            let mut c = Compound::new();
            c.insert("pos".into(), Tag::IntArray(l.pos.to_vec()));
            c.insert("face".into(), Tag::Byte(l.face as i8));
            c.insert(
                "kind".into(),
                Tag::Byte(match l.kind {
                    Kind::Extract => 0,
                    Kind::Insert => 1,
                }),
            );
            c.insert("channel".into(), Tag::String(l.channel.as_str().into()));
            Tag::Compound(c)
        })
        .collect();
    Ok([("labels".into(), Tag::List(list))].into())
}

fn decode_labels(data: &Compound) -> Result<Vec<Label>, AttachmentError> {
    let Some(Tag::List(list)) = data.get(&"labels".into()) else {
        return Err(AttachmentError::Decode);
    };
    let mut out = Vec::new();
    for tag in list {
        let Tag::Compound(c) = tag else {
            return Err(AttachmentError::Decode);
        };
        let (Some(Tag::IntArray(pos)), Some(Tag::Byte(face)), Some(Tag::Byte(kind)), Some(Tag::String(channel))) = (
            c.get(&"pos".into()),
            c.get(&"face".into()),
            c.get(&"kind".into()),
            c.get(&"channel".into()),
        ) else {
            return Err(AttachmentError::Decode);
        };
        if pos.len() != 3 || !(0..6).contains(face) {
            return Err(AttachmentError::Decode);
        }
        out.push(Label {
            pos: [pos[0], pos[1], pos[2]],
            face: *face as u8,
            kind: if *kind == 0 { Kind::Extract } else { Kind::Insert },
            channel: channel.to_string_lossy(),
        });
    }
    Ok(out)
}

/// A label's channel: its custom name as saved, or empty.
pub fn channel_of(stack: &ItemStack) -> String {
    stack
        .components()
        .and_then(|c| c.as_nbt().get(&"minecraft:custom_name".into()).cloned())
        .map(|name| match name {
            Tag::String(s) => s.to_string_lossy(),
            other => format!("{other:?}"),
        })
        .unwrap_or_default()
}

static KEYS: std::sync::OnceLock<[ItemKey; 2]> = std::sync::OnceLock::new();

fn kind_of(item: ItemKey) -> Option<Kind> {
    let keys = KEYS.get()?;
    if item == keys[0] {
        Some(Kind::Extract)
    } else if item == keys[1] {
        Some(Kind::Insert)
    } else {
        None
    }
}

struct LabelItem;

#[mod_api::horizon_part]
impl ItemPart for LabelItem {
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let kind = kind_of(event.stack.item())?;
        let face = face_of(event.face)?;
        let HostSide::Server(host) = host else {
            return None;
        };
        let level = host.level(host.dimension())?;
        let pos = event.pos;
        let side = DIRECTIONS[face as usize];
        level.capability(host, &ITEMS, pos, Some(side))?;
        let player = host.player(event.player)?;
        let sneaking = player.entity().is_shift_key_down(&*host) == Some(true);
        let chunk = Holder::Chunk([pos[0] >> 4, pos[2] >> 4]);
        let mut labels = LABELS.get(host.attachments(), chunk).ok()?;
        let before = labels.len();
        labels.retain(|l| !(l.pos == pos && l.face == face));
        let removed = labels.len() != before;
        if sneaking {
            if !removed {
                return None;
            }
            let _ = LABELS.set(host.attachments(), chunk, labels);
            return Some(InteractionResult::SuccessServer);
        }
        labels.push(Label {
            pos,
            face,
            kind,
            channel: channel_of(event.stack),
        });
        let _ = LABELS.set(host.attachments(), chunk, labels);
        if player.has_infinite_materials(&*host) != Some(true) {
            let living = LivingRef::new(player.entity());
            let _ = living.set_hand_item(host, event.hand, event.stack.clone().shrink(1));
        }
        Some(InteractionResult::SuccessServer)
    }
}

/// Transfers run so far, for the round robin.
static ROUND: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let keys = [
            r.item_key("transfer_labels:extract_label"),
            r.item_key("transfer_labels:insert_label"),
        ];
        for key in keys {
            r.item_part(key, LabelItem);
        }
        let _ = KEYS.set(keys);
    }

    fn level_tick_post(&self, host: HostSide<'_>, event: &mut LevelTickPostEvent) {
        let HostSide::Server(host) = host else {
            return;
        };
        let level = event.level;
        if level.game_time(&*host).unwrap_or(1).rem_euclid(PERIOD) != 0 {
            return;
        }
        let Ok(query) = host.query(&LABELS) else {
            return;
        };
        let labels: Vec<Label> = query
            .iter()
            .filter_map(Result::ok)
            .flat_map(|row| row.value.iter().cloned())
            .collect();
        drop(query);
        if labels.is_empty() {
            return;
        }
        let round = ROUND.fetch_add(1, Ordering::Relaxed);
        for (from_i, targets) in plan(&labels, round) {
            let from_label = &labels[from_i];
            let side = DIRECTIONS[from_label.face as usize];
            let Some(from) = level.capability(host, &ITEMS, from_label.pos, Some(side)) else {
                continue;
            };
            let mut left = ITEMS_PER_TRANSFER;
            for to_i in targets {
                if left == 0 {
                    break;
                }
                let to_label = &labels[to_i];
                let to_side = DIRECTIONS[to_label.face as usize];
                let Some(to) = level.capability(host, &ITEMS, to_label.pos, Some(to_side)) else {
                    continue;
                };
                let moved = host
                    .transaction(|host, tx| move_resources(host, &from, &to, |_| true, left, tx))
                    .unwrap_or(0);
                left -= moved.min(left);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(x: i32, kind: Kind, channel: &str) -> Label {
        Label {
            pos: [x, 64, 0],
            face: 1,
            kind,
            channel: channel.into(),
        }
    }

    #[test]
    fn extract_labels_send_only_to_insert_labels_of_their_channel_in_range() {
        let labels = vec![
            label(0, Kind::Extract, "ore"),
            label(5, Kind::Insert, "ore"),
            label(6, Kind::Insert, "wood"),
            label(40, Kind::Insert, "ore"),
            label(0, Kind::Insert, "ore"),
            label(7, Kind::Extract, "wood"),
        ];
        let p = plan(&labels, 0);
        assert_eq!(p, vec![(0, vec![1]), (5, vec![2])]);
    }

    #[test]
    fn each_round_starts_with_the_next_insert_label() {
        let labels = vec![
            label(0, Kind::Extract, ""),
            label(1, Kind::Insert, ""),
            label(2, Kind::Insert, ""),
            label(3, Kind::Insert, ""),
        ];
        let firsts: Vec<usize> = (0..4).map(|r| plan(&labels, r)[0].1[0]).collect();
        assert_eq!(firsts, [1, 2, 3, 1]);
    }

    #[test]
    fn labels_round_trip_through_their_saved_form() {
        let labels = vec![label(3, Kind::Extract, "a"), label(-9, Kind::Insert, "")];
        let saved = encode_labels(&labels).unwrap();
        assert_eq!(decode_labels(&saved).unwrap(), labels);
    }
}
