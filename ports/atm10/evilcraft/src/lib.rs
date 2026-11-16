//! EvilCraft for Horizon: blood. A mob that dies stains the block under it with its blood, and
//! the Blood Extractor draws that blood out into itself.
//!
//! Written from EvilCraft's public description (its in-game Book of Evil, the wiki, how the mod
//! plays), never from its code. What still has to be confirmed against the NeoForge original is
//! listed in README.md.
//!
//! How it works:
//! - `living_death`: the dying entity's blood ([`blood_of`] its max health) is added to a stain
//!   on the block it stood on, if that block's top is sturdy. Stains are kept per chunk
//!   (attachment `evilcraft:stains`, saved with the chunk) and hold up to [`STAIN_MAX`] mB.
//! - Right click on a stained block with `evilcraft:blood_extractor` moves the stain's blood
//!   into the extractor (custom data `evilcraft:blood`, up to [`EXTRACTOR_CAPACITY`] mB); an
//!   emptied stain disappears.
//!
//! The original swaps the block for a "blood stained" copy of itself; this port keeps the block
//! and records the stain beside it (README.md, "Needs from Horizon").
use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockFace, Component, Compound, Flow, Holder,
    HostSide, HostTransaction, InteractionResult, ItemPart, ItemPlace, LivingDeathEvent, Registrar,
    RegistrationContext, ServerHost, SupportType, Tag, Text, UseOnEvent,
};

/// Blood one point of max health gives, in mB.
pub const BLOOD_PER_HEALTH: i32 = 10;
/// The most one stained block holds, in mB.
pub const STAIN_MAX: i32 = 1000;
/// The Blood Extractor's tank, in mB.
pub const EXTRACTOR_CAPACITY: i32 = 5000;

/// The blood a death of a mob with `max_health` spills, in mB.
pub fn blood_of(max_health: f32) -> i32 {
    if !max_health.is_finite() || max_health <= 0.0 {
        return 0;
    }
    (max_health.ceil() as i32).saturating_mul(BLOOD_PER_HEALTH)
}

/// One stained block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stain {
    pub pos: [i32; 3],
    pub blood: i32,
}

/// Add `amount` of blood to the stain at `pos` in `stains`, making one if needed, up to
/// [`STAIN_MAX`]. Returns what the block took.
pub fn spill(stains: &mut Vec<Stain>, pos: [i32; 3], amount: i32) -> i32 {
    let i = match stains.iter().position(|s| s.pos == pos) {
        Some(i) => i,
        None => {
            stains.push(Stain { pos, blood: 0 });
            stains.len() - 1
        }
    };
    let took = amount.clamp(0, STAIN_MAX - stains[i].blood);
    stains[i].blood += took;
    if stains[i].blood == 0 {
        stains.remove(i);
    }
    took
}

/// Draw up to `room` mB from the stain at `pos`; an emptied stain is removed. Returns what came.
pub fn extract(stains: &mut Vec<Stain>, pos: [i32; 3], room: i32) -> i32 {
    let Some(i) = stains.iter().position(|s| s.pos == pos) else {
        return 0;
    };
    let took = stains[i].blood.min(room.max(0));
    stains[i].blood -= took;
    if stains[i].blood == 0 {
        stains.remove(i);
    }
    took
}

/// The stains on one chunk, saved as `{stains: [I; x, y, z, blood, ...]}`.
static STAINS: AttachmentType<Vec<Stain>> = AttachmentType {
    namespace: "evilcraft",
    name: "stains",
    default: Vec::new,
    codec: Some(AttachmentCodec {
        encode: encode_stains,
        decode: decode_stains,
    }),
    copy_on_death: false,
    sync: false,
};

fn encode_stains(stains: &Vec<Stain>) -> Result<Compound, AttachmentError> {
    let flat = stains
        .iter()
        .flat_map(|s| [s.pos[0], s.pos[1], s.pos[2], s.blood])
        .collect();
    Ok([("stains".into(), Tag::IntArray(flat))].into())
}

fn decode_stains(data: &Compound) -> Result<Vec<Stain>, AttachmentError> {
    let Some(Tag::IntArray(flat)) = data.get(&"stains".into()) else {
        return Err(AttachmentError::Decode);
    };
    if flat.len() % 4 != 0 {
        return Err(AttachmentError::Decode);
    }
    Ok(flat
        .chunks_exact(4)
        .map(|c| Stain {
            pos: [c[0], c[1], c[2]],
            blood: c[3].clamp(0, STAIN_MAX),
        })
        .filter(|s| s.blood > 0)
        .collect())
}

fn chunk_of(pos: [i32; 3]) -> Holder {
    Holder::Chunk([pos[0] >> 4, pos[2] >> 4])
}

/// The extractor keeps its blood in `minecraft:custom_data`.
struct CustomData;

impl Component for CustomData {
    type Value = Compound;
    fn id(&self) -> &str {
        "minecraft:custom_data"
    }
    fn decode(&self, value: &Tag) -> Option<Compound> {
        match value {
            Tag::Compound(c) => Some(c.clone()),
            _ => None,
        }
    }
    fn encode(&self, value: Compound) -> Tag {
        Tag::Compound(value)
    }
}

const BLOOD: &str = "evilcraft:blood";

/// The blood in an extractor stack, in mB.
fn stored(stack: &mod_api::ItemStack) -> i32 {
    match stack
        .get(&CustomData)
        .and_then(|d| d.get(&BLOOD.into()).cloned())
    {
        Some(Tag::Int(v)) => v.clamp(0, EXTRACTOR_CAPACITY),
        _ => 0,
    }
}

struct Extractor;

impl Extractor {
    fn draw(host: &mut dyn ServerHost, event: &UseOnEvent<'_>) -> Option<InteractionResult> {
        let pos = event.pos;
        let holder = chunk_of(pos);
        let mut stains = STAINS.get(host.attachments(), holder).ok()?;
        if !stains.iter().any(|s| s.pos == pos) {
            return None;
        }
        let have = stored(event.stack);
        let took = extract(&mut stains, pos, EXTRACTOR_CAPACITY - have);
        let player = host.player(event.player)?;
        if took == 0 {
            let _ = player.action_bar(
                host,
                Text::translate("message.evilcraft.blood_extractor.full", vec![]),
            );
            return Some(InteractionResult::Fail);
        }
        let place = ItemPlace::PlayerHand {
            player: event.player,
            hand: event.hand,
        };
        let total = have + took;
        let edited: Result<bool, ()> = host.transaction(|host, tx| {
            Ok(place.edit(host, tx, |s| {
                let mut data = s.get(&CustomData).unwrap_or_default();
                data.insert(BLOOD.into(), Tag::Int(total));
                let _ = s.set(&CustomData, data);
            }))
        });
        if edited != Ok(true) {
            return Some(InteractionResult::Fail);
        }
        STAINS.set(host.attachments(), holder, stains).ok()?;
        let _ = player.action_bar(
            host,
            Text::translate(
                "message.evilcraft.blood_extractor.amount",
                vec![
                    Text::literal(total.to_string()),
                    Text::literal(EXTRACTOR_CAPACITY.to_string()),
                ],
            ),
        );
        if let Some(level) = player.level(host) {
            let at = [
                pos[0] as f64 + 0.5,
                pos[1] as f64 + 1.0,
                pos[2] as f64 + 0.5,
            ];
            level.play_sound(
                host,
                "minecraft:item.bucket.fill",
                at,
                mod_api::SoundSource::Players,
                0.6,
                0.8,
            );
        }
        Some(InteractionResult::SuccessServer)
    }
}

#[mod_api::horizon_part]
impl ItemPart for Extractor {
    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        match host {
            HostSide::Server(host) => Extractor::draw(host, event),
            HostSide::Client(_) => None,
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl mod_api::Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let extractor = r.item_key("evilcraft:blood_extractor");
        r.item_part(extractor, Extractor);
    }

    /// A death spills the body's blood on the block under it.
    fn living_death(&self, host: &mut dyn ServerHost, event: &mut LivingDeathEvent<'_>) -> Flow {
        if event.cancelled {
            return Flow::Continue;
        }
        let Some(entity) = host.entity(event.entity) else {
            return Flow::Continue;
        };
        let Some(blood) = host
            .living(entity)
            .and_then(|l| l.max_health(host))
            .map(blood_of)
        else {
            return Flow::Continue;
        };
        let (Some(level), Some(below)) = (entity.level(host), entity.on_block(host)) else {
            return Flow::Continue;
        };
        if level.block_face_sturdy(host, below, BlockFace::Up, SupportType::Full) != Some(true) {
            return Flow::Continue;
        }
        let holder = chunk_of(below);
        if let Ok(mut stains) = STAINS.get(host.attachments(), holder) {
            if spill(&mut stains, below, blood) > 0 {
                let _ = STAINS.set(host.attachments(), holder, stains);
            }
        }
        Flow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zombie_spills_two_hundred_and_a_stain_caps_at_a_thousand() {
        let mut stains = Vec::new();
        let zombie = blood_of(20.0);
        assert_eq!(zombie, 200);
        for _ in 0..5 {
            spill(&mut stains, [1, 64, 1], zombie);
        }
        assert_eq!(spill(&mut stains, [1, 64, 1], zombie), 0);
        assert_eq!(
            stains,
            vec![Stain {
                pos: [1, 64, 1],
                blood: 1000
            }]
        );
    }

    #[test]
    fn extracting_drains_and_then_clears_the_stain() {
        let mut stains = vec![
            Stain {
                pos: [0, 70, 0],
                blood: 700,
            },
            Stain {
                pos: [3, 70, 0],
                blood: 50,
            },
        ];
        assert_eq!(extract(&mut stains, [0, 70, 0], 500), 500);
        assert_eq!(stains[0].blood, 200);
        assert_eq!(extract(&mut stains, [0, 70, 0], 500), 200);
        assert_eq!(
            stains,
            vec![Stain {
                pos: [3, 70, 0],
                blood: 50
            }]
        );
        assert_eq!(extract(&mut stains, [9, 9, 9], 500), 0);
    }

    #[test]
    fn stains_round_trip_through_their_saved_form() {
        let stains = vec![
            Stain {
                pos: [-4, 12, 99],
                blood: 30,
            },
            Stain {
                pos: [7, -50, 2],
                blood: 1000,
            },
        ];
        let saved = encode_stains(&stains).unwrap();
        assert_eq!(decode_stains(&saved).unwrap(), stains);
    }
}
