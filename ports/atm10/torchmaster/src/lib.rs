//! Torchmaster for Horizon: the Mega Torch stops hostile mobs from spawning naturally around it,
//! the Dread Lamp does the same for passive animals.
//!
//! Written from Torchmaster's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and what a harness against the NeoForge
//! original still has to confirm, is in README.md.
//!
//! How it works: placing one of the two blocks records its position on the chunk it stands in
//! (a chunk attachment, saved with the chunk). Every spawn check reads those records from the
//! level's loaded chunks and refuses a natural spawn of the matching mob category inside the
//! block's cube. A record whose block has gone (broken, exploded, pushed) is ignored: the check
//! confirms the block is still there before it refuses, so no break hook is needed.
use std::sync::OnceLock;

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentQueries, AttachmentType, BlockKey, Compound,
    Holder, Hooks, Host, ItemPart, MobCategory, MobSpawnPlacementCheckEvent, OnBlockPlacedByEvent,
    Registrar, RegistrationContext, ServerHost, SpawnCheck, SpawnReason, Tag,
};

/// Torchmaster's default radius for both blocks, in blocks along each axis.
pub const RADIUS: i32 = 64;

/// Which light a record is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `torchmaster:megatorch`: blocks `MobCategory::Monster`.
    MegaTorch,
    /// `torchmaster:dreadlamp`: blocks `MobCategory::Creature`.
    DreadLamp,
}

impl Kind {
    fn id(self) -> &'static str {
        match self {
            Kind::MegaTorch => "torchmaster:megatorch",
            Kind::DreadLamp => "torchmaster:dreadlamp",
        }
    }
    fn code(self) -> i32 {
        match self {
            Kind::MegaTorch => 0,
            Kind::DreadLamp => 1,
        }
    }
    fn from_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Kind::MegaTorch),
            1 => Some(Kind::DreadLamp),
            _ => None,
        }
    }
    /// The light that stops a mob of `category`, if any.
    pub fn for_category(category: MobCategory) -> Option<Self> {
        match category {
            MobCategory::Monster => Some(Kind::MegaTorch),
            MobCategory::Creature => Some(Kind::DreadLamp),
            _ => None,
        }
    }
    /// Whether this light stops a spawn for `reason`. Observed on the original: the Mega Torch
    /// blocks natural, patrol and chunk-generation spawns, not events. The Dread Lamp's reasons
    /// are not observed yet; natural only.
    pub fn blocks(self, reason: SpawnReason) -> bool {
        match self {
            Kind::MegaTorch => matches!(
                reason,
                SpawnReason::Natural | SpawnReason::Patrol | SpawnReason::ChunkGeneration
            ),
            Kind::DreadLamp => matches!(reason, SpawnReason::Natural),
        }
    }
}

/// One placed light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Light {
    pub pos: [i32; 3],
    pub kind: Kind,
}

impl Light {
    /// Whether `pos` lies in this light's cube.
    pub fn covers(&self, pos: [i32; 3]) -> bool {
        // Measured on the original: a cube, 64 blocks to each side, one block more upwards.
        let d = [pos[0] - self.pos[0], pos[1] - self.pos[1], pos[2] - self.pos[2]];
        d[0].abs() <= RADIUS && d[2].abs() <= RADIUS && (-RADIUS..=RADIUS + 1).contains(&d[1])
    }
}

/// The lights recorded on one chunk, saved as `{lights: [I; x, y, z, kind, ...]}`.
static LIGHTS: AttachmentType<Vec<Light>> = AttachmentType {
    namespace: "torchmaster",
    name: "lights",
    default: Vec::new,
    codec: Some(AttachmentCodec {
        encode: encode_lights,
        decode: decode_lights,
    }),
    copy_on_death: false,
    sync: false,
};

fn encode_lights(lights: &Vec<Light>) -> Result<Compound, AttachmentError> {
    let flat = lights
        .iter()
        .flat_map(|l| [l.pos[0], l.pos[1], l.pos[2], l.kind.code()])
        .collect();
    Ok([("lights".into(), Tag::IntArray(flat))].into())
}

fn decode_lights(data: &Compound) -> Result<Vec<Light>, AttachmentError> {
    let Some(Tag::IntArray(flat)) = data.get(&"lights".into()) else {
        return Err(AttachmentError::Decode);
    };
    if flat.len() % 4 != 0 {
        return Err(AttachmentError::Decode);
    }
    Ok(flat
        .chunks_exact(4)
        .filter_map(|c| {
            Some(Light {
                pos: [c[0], c[1], c[2]],
                kind: Kind::from_code(c[3])?,
            })
        })
        .collect())
}

/// The two blocks' keys, known once registration ran.
static KEYS: OnceLock<[BlockKey; 2]> = OnceLock::new();

/// The Mega Torch's wall form: the item places it on a wall, and it is the same light.
static WALL: OnceLock<BlockKey> = OnceLock::new();

fn kind_of(block: BlockKey) -> Option<Kind> {
    if WALL.get() == Some(&block) {
        return Some(Kind::MegaTorch);
    }
    let keys = KEYS.get()?;
    [Kind::MegaTorch, Kind::DreadLamp]
        .into_iter()
        .zip(keys)
        .find_map(|(kind, key)| (*key == block).then_some(kind))
}

fn key_of(kind: Kind) -> Option<BlockKey> {
    KEYS.get().map(|k| k[kind.code() as usize])
}

/// The two blocks' items: placing one records the light.
struct LightItem;

#[mod_api::horizon_part]
impl ItemPart for LightItem {
    /// The item placed its block (`BlockItem`'s `onBlockPlacedBy`): record it on its chunk.
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        let Some(kind) = kind_of(event.state.block()) else {
            return;
        };
        let pos = event.pos;
        let chunk = Holder::Chunk([pos[0] >> 4, pos[2] >> 4]);
        let Ok(mut lights) = LIGHTS.get(host.attachments(), chunk) else {
            return;
        };
        lights.retain(|l| l.pos != pos);
        lights.push(Light { pos, kind });
        let _ = LIGHTS.set(host.attachments(), chunk, lights);
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let keys = [
            r.block_key(Kind::MegaTorch.id()),
            r.block_key(Kind::DreadLamp.id()),
        ];
        let _ = KEYS.set(keys);
        let _ = WALL.set(r.block_key("torchmaster:megatorch_wall"));
        for kind in [Kind::MegaTorch, Kind::DreadLamp] {
            let item = r.item_key(kind.id());
            r.item_part(item, LightItem);
        }
    }

    /// Refuse a natural spawn inside a light's cube.
    fn mob_spawn_placement_check(
        &self,
        host: &dyn Host,
        event: &mut MobSpawnPlacementCheckEvent<'_>,
    ) {
        if *event.result != SpawnCheck::Default || !event.default_result {
            return;
        }
        let Some(kind) = host
            .entity_type_category(event.kind)
            .and_then(Kind::for_category)
        else {
            return;
        };
        if !kind.blocks(event.reason) {
            return;
        }
        let Some(block) = key_of(kind) else {
            return;
        };
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        let Ok(query) = host.query(&LIGHTS) else {
            return;
        };
        let covered = query.iter().filter_map(Result::ok).any(|row| {
            row.value.iter().any(|light| {
                light.kind == kind
                    && light.covers(pos)
                    && matches!(event.level.block(host, light.pos),
                        Some(b) if b == block || (kind == Kind::MegaTorch && WALL.get() == Some(&b)))
            })
        });
        if covered {
            *event.result = SpawnCheck::Fail;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cube_reaches_64_blocks_on_each_axis() {
        let torch = Light {
            pos: [0, 64, 0],
            kind: Kind::MegaTorch,
        };
        assert!(torch.covers([64, 0, -64]));
        assert!(torch.covers([-64, 129, 64]));
        assert!(!torch.covers([0, 130, 0]));
        assert!(!torch.covers([65, 64, 0]));
        assert!(!torch.covers([0, -1, 65]));
    }

    #[test]
    fn lights_round_trip_through_their_saved_form() {
        let lights = vec![
            Light {
                pos: [1, -60, 3],
                kind: Kind::MegaTorch,
            },
            Light {
                pos: [-17, 200, 40],
                kind: Kind::DreadLamp,
            },
        ];
        let saved = encode_lights(&lights).unwrap();
        assert_eq!(decode_lights(&saved).unwrap(), lights);
    }

    #[test]
    fn each_light_stops_only_its_category_and_reasons() {
        assert_eq!(
            Kind::for_category(MobCategory::Monster),
            Some(Kind::MegaTorch)
        );
        assert_eq!(
            Kind::for_category(MobCategory::Creature),
            Some(Kind::DreadLamp)
        );
        assert_eq!(Kind::for_category(MobCategory::Ambient), None);
        assert!(Kind::MegaTorch.blocks(SpawnReason::Natural));
        assert!(!Kind::MegaTorch.blocks(SpawnReason::Spawner));
        assert!(!Kind::MegaTorch.blocks(SpawnReason::Event));
        assert!(Kind::MegaTorch.blocks(SpawnReason::Patrol));
        assert!(!Kind::DreadLamp.blocks(SpawnReason::Event));
    }
}
