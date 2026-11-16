//! FramedBlocks for Horizon: the Framed Cube's camouflage.
//!
//! Written from FramedBlocks' observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and what a harness against the NeoForge
//! original still has to confirm, is in README.md.
//!
//! How it works: right-clicking a Framed Cube with a block puts that block in the frame as its
//! camouflage (one item is used up, not in creative). Glowstone dust makes the frame glow (its
//! `glowing` state, light 15). The Framed Hammer takes the camouflage and the glowstone back out.
//! Breaking the frame drops what it held. The camouflage is recorded on the frame's chunk (a
//! chunk attachment, saved with the chunk); the glow is the block's own state.
use std::sync::OnceLock;

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockKey, BlockPart, BlockState,
    BlockUpdateFlags, Compound, Hand, Holder, Hooks, HostBlockStates, InteractionResult, ItemKey,
    ItemStack, LevelRef, OnDestroyedByPlayerEvent, PlayerRef, Registrar, RegistrationContext,
    ServerHost, Tag, UseItemOnEvent,
};

/// One frame's contents.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Frame {
    /// The camouflage block's id, or None for a bare frame.
    pub camo: Option<String>,
    /// Whether glowstone dust was applied.
    pub glowing: bool,
}

/// What the player right-clicks the frame with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Held {
    /// A block that may camouflage the frame, by id.
    Block(String),
    /// Glowstone dust.
    Glowstone,
    /// The Framed Hammer.
    Hammer,
    /// Anything else, or an empty hand.
    Other,
}

/// Something the frame gives back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Returned {
    /// The camouflage block's item, by block id.
    Camo(String),
    /// One glowstone dust.
    Glowstone,
}

/// What a click does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The frame after the click.
    pub frame: Frame,
    /// Whether the held stack loses one item (a survival player's camouflage or glowstone).
    pub consume: bool,
    /// What goes back to the player.
    pub returned: Vec<Returned>,
}

/// A click on `frame` with `held`; None when the click does nothing (it passes on, so a block
/// that can't go in the frame is placed against it as usual).
pub fn click(frame: &Frame, held: &Held, creative: bool) -> Option<Outcome> {
    match held {
        Held::Block(id) if frame.camo.is_none() => Some(Outcome {
            frame: Frame {
                camo: Some(id.clone()),
                glowing: frame.glowing,
            },
            consume: !creative,
            returned: Vec::new(),
        }),
        Held::Glowstone if !frame.glowing => Some(Outcome {
            frame: Frame {
                camo: frame.camo.clone(),
                glowing: true,
            },
            consume: !creative,
            returned: Vec::new(),
        }),
        Held::Hammer if *frame != Frame::default() => Some(Outcome {
            frame: Frame::default(),
            consume: false,
            returned: if creative {
                Vec::new()
            } else {
                contents(frame)
            },
        }),
        _ => None,
    }
}

/// What a frame holds, as it drops when the frame is broken or emptied.
pub fn contents(frame: &Frame) -> Vec<Returned> {
    let mut out: Vec<Returned> = frame.camo.iter().cloned().map(Returned::Camo).collect();
    if frame.glowing {
        out.push(Returned::Glowstone);
    }
    out
}

/// The camouflage of the frames on one chunk: `{frames: [{pos: [I; x, y, z], camo: "id"}, ...]}`.
static CAMO: AttachmentType<Vec<([i32; 3], String)>> = AttachmentType {
    namespace: "framedblocks",
    name: "camo",
    default: Vec::new,
    codec: Some(AttachmentCodec {
        encode: encode_camo,
        decode: decode_camo,
    }),
    copy_on_death: false,
    sync: false,
};

fn encode_camo(frames: &Vec<([i32; 3], String)>) -> Result<Compound, AttachmentError> {
    let list = frames
        .iter()
        .map(|(pos, camo)| {
            Tag::Compound(
                [
                    ("pos".into(), Tag::IntArray(pos.to_vec())),
                    ("camo".into(), Tag::String(camo.as_str().into())),
                ]
                .into(),
            )
        })
        .collect();
    Ok([("frames".into(), Tag::List(list))].into())
}

fn decode_camo(data: &Compound) -> Result<Vec<([i32; 3], String)>, AttachmentError> {
    let Some(Tag::List(list)) = data.get(&"frames".into()) else {
        return Err(AttachmentError::Decode);
    };
    list.iter()
        .map(|entry| {
            let Tag::Compound(entry) = entry else {
                return Err(AttachmentError::Decode);
            };
            let (Some(Tag::IntArray(pos)), Some(Tag::String(camo))) =
                (entry.get(&"pos".into()), entry.get(&"camo".into()))
            else {
                return Err(AttachmentError::Decode);
            };
            let [x, y, z] = pos[..] else {
                return Err(AttachmentError::Decode);
            };
            Ok(([x, y, z], camo.to_string_lossy()))
        })
        .collect()
}

struct Keys {
    frame: BlockKey,
    hammer: ItemKey,
    glowstone: ItemKey,
}

static KEYS: OnceLock<Keys> = OnceLock::new();

fn chunk_of(pos: [i32; 3]) -> Holder {
    Holder::Chunk([pos[0] >> 4, pos[2] >> 4])
}

fn read_camo(host: &mut dyn ServerHost, pos: [i32; 3]) -> Option<String> {
    let frames = CAMO.get(host.attachments(), chunk_of(pos)).ok()?;
    frames.into_iter().find(|(p, _)| *p == pos).map(|(_, c)| c)
}

fn write_camo(host: &mut dyn ServerHost, pos: [i32; 3], camo: Option<String>) {
    let Ok(mut frames) = CAMO.get(host.attachments(), chunk_of(pos)) else {
        return;
    };
    frames.retain(|(p, _)| *p != pos);
    if let Some(camo) = camo {
        frames.push((pos, camo));
    }
    let _ = CAMO.set(host.attachments(), chunk_of(pos), frames);
}

fn is_glowing(host: &dyn ServerHost, state: BlockState) -> bool {
    state
        .property(&host.block_states(), "glowing")
        .ok()
        .flatten()
        == Some("true")
}

/// What `stack` is to a frame.
fn held(host: &dyn ServerHost, stack: Option<&ItemStack>) -> Held {
    let (Some(stack), Some(keys)) = (stack, KEYS.get()) else {
        return Held::Other;
    };
    let item = stack.item();
    if item == keys.hammer {
        return Held::Hammer;
    }
    if item == keys.glowstone {
        return Held::Glowstone;
    }
    let Some(block) = host.item_block(item) else {
        return Held::Other;
    };
    let Ok(state) = BlockState::of(&host.block_states(), block, &[]) else {
        return Held::Other;
    };
    // A frame can't hold itself, nor a block that keeps a block entity (a chest, a furnace) or
    // that is replaced when placed against (grass, snow layers).
    let facts = host.block_states();
    if block == keys.frame
        || facts.has_block_entity(state) != Some(false)
        || facts.replaceable(state) != Some(false)
    {
        return Held::Other;
    }
    Held::Block(block.id(host))
}

fn stack_of(host: &dyn ServerHost, returned: &Returned) -> Option<ItemStack> {
    match returned {
        Returned::Glowstone => KEYS.get().map(|k| ItemStack::of(k.glowstone)),
        Returned::Camo(id) => {
            let block = host.block_key(id)?;
            host.block_item(block).map(ItemStack::of)
        }
    }
}

fn give(host: &mut dyn ServerHost, player: PlayerRef, returned: &[Returned]) {
    for r in returned {
        if let Some(stack) = stack_of(host, r) {
            let _ = player.give_stack(host, stack);
        }
    }
}

fn drop_at(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], returned: &[Returned]) {
    for r in returned {
        if let Some(stack) = stack_of(host, r) {
            let _ = level.spawn_item(
                host,
                pos[0] as f64 + 0.5,
                pos[1] as f64 + 0.5,
                pos[2] as f64 + 0.5,
                stack,
                10,
            );
        }
    }
}

/// The Framed Cube.
struct FramedCube;

#[mod_api::horizon_part]
impl BlockPart for FramedCube {
    fn use_item_on(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseItemOnEvent<'_>,
    ) -> Option<InteractionResult> {
        let click_ = event.click;
        let pos = click_.pos;
        let player = host.player(click_.player)?;
        let level = player.level(host)?;
        let creative = player.is_creative(host).unwrap_or(false);
        let frame = Frame {
            camo: read_camo(host, pos),
            glowing: is_glowing(host, event.state),
        };
        let what = held(host, click_.stack);
        let outcome = click(&frame, &what, creative)?;
        if outcome.frame.glowing != frame.glowing {
            let glow = if outcome.frame.glowing {
                "true"
            } else {
                "false"
            };
            let state = event
                .state
                .with(&host.block_states(), "glowing", glow)
                .ok()?;
            level
                .set_block(
                    host,
                    pos,
                    state,
                    BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS,
                )
                .ok()?;
        }
        if outcome.frame.camo != frame.camo {
            write_camo(host, pos, outcome.frame.camo.clone());
            // The camouflage changes how the frame looks: send it again.
            level.send_block_updated(host, pos);
        }
        if outcome.consume
            && let Some(stack) = click_.stack
        {
            let hand: Hand = click_.hand;
            if let Some(living) = host.living(player.entity()) {
                let _ = living.set_hand_item(host, hand, stack.clone().shrink(1));
            }
        }
        give(host, player, &outcome.returned);
        Some(InteractionResult::Success)
    }

    /// The frame is broken: what it held drops with it (not in creative).
    fn on_destroyed_by_player(
        &self,
        host: &mut dyn ServerHost,
        event: &mut OnDestroyedByPlayerEvent<'_>,
    ) -> Option<bool> {
        let frame = Frame {
            camo: read_camo(host, event.pos),
            glowing: is_glowing(host, event.state),
        };
        write_camo(host, event.pos, None);
        let player = host.player(event.player)?;
        let creative = player.is_creative(host).unwrap_or(false);
        if event.will_harvest && !creative {
            let level = player.level(host)?;
            drop_at(host, level, event.pos, &contents(&frame));
        }
        None
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let frame = r.block_key("framedblocks:framed_cube");
        let _ = KEYS.set(Keys {
            frame,
            hammer: r.item_key("framedblocks:framed_hammer"),
            glowstone: r.item_key("minecraft:glowstone_dust"),
        });
        r.block_part(frame, FramedCube);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stone() -> Held {
        Held::Block("minecraft:stone".into())
    }

    #[test]
    fn a_block_camouflages_a_bare_frame_once() {
        let bare = Frame::default();
        let done = click(&bare, &stone(), false).unwrap();
        assert_eq!(done.frame.camo.as_deref(), Some("minecraft:stone"));
        assert!(done.consume);
        // A second block does nothing: it is placed against the frame instead.
        assert_eq!(
            click(&done.frame, &Held::Block("minecraft:dirt".into()), false),
            None
        );
        // In creative the block stays in hand.
        assert!(!click(&bare, &stone(), true).unwrap().consume);
    }

    #[test]
    fn the_hammer_gives_back_the_camouflage_and_the_glowstone() {
        let frame = Frame {
            camo: Some("minecraft:oak_planks".into()),
            glowing: true,
        };
        let out = click(&frame, &Held::Hammer, false).unwrap();
        assert_eq!(out.frame, Frame::default());
        assert_eq!(
            out.returned,
            vec![
                Returned::Camo("minecraft:oak_planks".into()),
                Returned::Glowstone
            ]
        );
        assert!(!out.consume);
        // An empty frame has nothing to give.
        assert_eq!(click(&Frame::default(), &Held::Hammer, false), None);
    }

    #[test]
    fn glowstone_lights_a_frame_once_and_keeps_its_camouflage() {
        let frame = Frame {
            camo: Some("minecraft:stone".into()),
            glowing: false,
        };
        let lit = click(&frame, &Held::Glowstone, false).unwrap();
        assert!(lit.frame.glowing);
        assert_eq!(lit.frame.camo, frame.camo);
        assert_eq!(click(&lit.frame, &Held::Glowstone, false), None);
    }

    #[test]
    fn the_camouflage_survives_its_saved_form() {
        let frames = vec![
            ([1, -60, 3], "minecraft:stone".to_owned()),
            ([-17, 200, 40], "minecraft:deepslate_tiles".to_owned()),
        ];
        let saved = encode_camo(&frames).unwrap();
        assert_eq!(decode_camo(&saved).unwrap(), frames);
    }
}
