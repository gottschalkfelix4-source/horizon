//! Restrictions for Horizon: three blocks that move entities.
//!
//! - Attractor: pulls entities in front of it towards it, as far as the redstone signal it gets
//!   (0 to 15 blocks); off without a signal.
//! - Pusher: the same, pushing them away.
//! - One Way: a see-through block entities pass through, moved along its facing, with no
//!   redstone (an elevator when it faces up).
//!
//! Written from Restrictions' public description and observed behaviour, never from its code
//! (docs/modpacks.md, "Licences"). See README.md for what is assumed.
//!
//! How: a placed block faces the player who placed it (the One Way faces where they look).
//! Placing an Attractor or a Pusher from its item records it on its chunk (attachment
//! `restrictions:devices`, saved with the chunk); after each level tick the recorded devices
//! still standing and powered move the entities in their beam. The One Way's part answers
//! `entity_inside` with the entity's new movement.
use std::sync::OnceLock;

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentQueries, AttachmentType, BlockKey, BlockPart,
    BlockState, Compound, EntityInsideEvent, EntityWrite, Holder, Hooks, HostSide, InsideEffect,
    ItemPart, LevelHost, LevelRandomHost, LevelRef, LevelTickPostEvent, OnBlockPlacedByEvent,
    Registrar, RegistrationContext, ServerHost, StateForPlacementEvent, Tag,
};

/// How fast the One Way moves what is inside it, in blocks per tick.
pub const ONE_WAY_SPEED: f64 = 0.4;
/// What a powered device adds to an entity's speed each tick, in blocks per tick.
pub const DEVICE_ACCEL: f64 = 0.08;
/// The speed along the beam a device does not push past.
pub const DEVICE_MAX_SPEED: f64 = 0.6;

/// A block face, as the `facing` state property names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Facing {
    pub const ALL: [Facing; 6] = [
        Facing::Down,
        Facing::Up,
        Facing::North,
        Facing::South,
        Facing::West,
        Facing::East,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Facing::Down => "down",
            Facing::Up => "up",
            Facing::North => "north",
            Facing::South => "south",
            Facing::West => "west",
            Facing::East => "east",
        }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.name() == name)
    }
    pub fn step(self) -> [i32; 3] {
        match self {
            Facing::Down => [0, -1, 0],
            Facing::Up => [0, 1, 0],
            Facing::North => [0, 0, -1],
            Facing::South => [0, 0, 1],
            Facing::West => [-1, 0, 0],
            Facing::East => [1, 0, 0],
        }
    }
    pub fn opposite(self) -> Self {
        match self {
            Facing::Down => Facing::Up,
            Facing::Up => Facing::Down,
            Facing::North => Facing::South,
            Facing::South => Facing::North,
            Facing::West => Facing::East,
            Facing::East => Facing::West,
        }
    }
    /// The face a look of `yaw`/`pitch` (degrees, vanilla's convention) points at most:
    /// `Direction.orderedByNearest(entity)[0]`.
    pub fn looking(yaw: f32, pitch: f32) -> Self {
        let (yaw, pitch) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
        let look = [
            -yaw.sin() * pitch.cos(),
            -pitch.sin(),
            yaw.cos() * pitch.cos(),
        ];
        Self::ALL
            .into_iter()
            .max_by(|a, b| {
                let dot = |f: Facing| {
                    let s = f.step();
                    look[0] * f64::from(s[0])
                        + look[1] * f64::from(s[1])
                        + look[2] * f64::from(s[2])
                };
                dot(*a).total_cmp(&dot(*b))
            })
            .expect("six faces")
    }
}

/// The two powered devices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Attractor,
    Pusher,
}

impl Kind {
    fn id(self) -> &'static str {
        match self {
            Kind::Attractor => "restrictions:attractor",
            Kind::Pusher => "restrictions:pusher",
        }
    }
    fn code(self) -> i32 {
        match self {
            Kind::Attractor => 0,
            Kind::Pusher => 1,
        }
    }
    fn from_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Kind::Attractor),
            1 => Some(Kind::Pusher),
            _ => None,
        }
    }
}

/// One recorded device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Device {
    pub pos: [i32; 3],
    pub kind: Kind,
}

/// The box in front of a block facing `facing`, `range` blocks deep and one block wide:
/// `[min, max]`.
pub fn beam(pos: [i32; 3], facing: Facing, range: i32) -> [[f64; 3]; 2] {
    let step = facing.step();
    let near = pos.map(f64::from);
    let mut min = near;
    let mut max = near.map(|c| c + 1.0);
    for axis in 0..3 {
        match step[axis] {
            1 => {
                min[axis] = near[axis] + 1.0;
                max[axis] = near[axis] + 1.0 + f64::from(range);
            }
            -1 => {
                min[axis] = near[axis] - f64::from(range);
                max[axis] = near[axis];
            }
            _ => {}
        }
    }
    [min, max]
}

/// The push a device gives an entity moving at `velocity`: along the beam, towards the device
/// for an Attractor, away for a Pusher, until the entity moves that way at the cap.
pub fn device_push(kind: Kind, facing: Facing, velocity: [f64; 3]) -> Option<[f64; 3]> {
    let away = facing.step().map(f64::from);
    let sign = match kind {
        Kind::Attractor => -1.0,
        Kind::Pusher => 1.0,
    };
    let along: f64 = (0..3).map(|i| velocity[i] * away[i] * sign).sum();
    (along < DEVICE_MAX_SPEED).then(|| away.map(|c| c * sign * DEVICE_ACCEL))
}

/// The movement the One Way gives what is inside it.
pub fn one_way_motion(facing: Facing) -> [f64; 3] {
    facing.step().map(|c| f64::from(c) * ONE_WAY_SPEED)
}

static DEVICES: AttachmentType<Vec<Device>> = AttachmentType {
    namespace: "restrictions",
    name: "devices",
    default: Vec::new,
    codec: Some(AttachmentCodec {
        encode: encode_devices,
        decode: decode_devices,
    }),
    copy_on_death: false,
    sync: false,
};

// The codec's signature is `fn(&T)` with `T = Vec<Device>`.
#[allow(clippy::ptr_arg)]
fn encode_devices(devices: &Vec<Device>) -> Result<Compound, AttachmentError> {
    let flat = devices
        .iter()
        .flat_map(|d| [d.pos[0], d.pos[1], d.pos[2], d.kind.code()])
        .collect();
    Ok([("devices".into(), Tag::IntArray(flat))].into())
}

fn decode_devices(data: &Compound) -> Result<Vec<Device>, AttachmentError> {
    let Some(Tag::IntArray(flat)) = data.get(&"devices".into()) else {
        return Err(AttachmentError::Decode);
    };
    if flat.len() % 4 != 0 {
        return Err(AttachmentError::Decode);
    }
    Ok(flat
        .chunks_exact(4)
        .filter_map(|c| {
            Some(Device {
                pos: [c[0], c[1], c[2]],
                kind: Kind::from_code(c[3])?,
            })
        })
        .collect())
}

/// The blocks' keys: attractor, pusher, one way.
static KEYS: OnceLock<[BlockKey; 3]> = OnceLock::new();

fn kind_of(block: BlockKey) -> Option<Kind> {
    let keys = KEYS.get()?;
    if block == keys[0] {
        Some(Kind::Attractor)
    } else if block == keys[1] {
        Some(Kind::Pusher)
    } else {
        None
    }
}

fn facing_of(level: &dyn LevelHost, state: BlockState) -> Option<Facing> {
    let text = level.state_string(state)?;
    let inner = text.split_once('[')?.1.trim_end_matches(']');
    inner
        .split(',')
        .find_map(|kv| kv.strip_prefix("facing="))
        .and_then(Facing::from_name)
}

/// Every block: face the way it was placed.
struct Placed {
    /// True for the One Way (faces where the player looks), false for the devices (face the
    /// player, as a piston does).
    along_look: bool,
}

#[mod_api::horizon_part]
impl BlockPart for Placed {
    fn state_for_placement(
        &self,
        level: &dyn LevelHost,
        event: &mut StateForPlacementEvent<'_>,
    ) -> Option<Option<BlockState>> {
        let look = Facing::looking(event.context.yaw, event.context.pitch);
        let facing = if self.along_look {
            look
        } else {
            look.opposite()
        };
        level
            .state_with_property(event.state, "facing", facing.name())
            .map(Some)
    }
}

/// The One Way: what is inside moves along its facing.
struct OneWay;

#[mod_api::horizon_part]
impl BlockPart for OneWay {
    fn state_for_placement(
        &self,
        level: &dyn LevelHost,
        event: &mut StateForPlacementEvent<'_>,
    ) -> Option<Option<BlockState>> {
        Placed { along_look: true }.state_for_placement(level, event)
    }

    fn entity_inside(&self, level: &mut dyn LevelRandomHost, event: &mut EntityInsideEvent<'_>) {
        let Some(facing) = facing_of(level, event.state) else {
            return;
        };
        event.effects.run_before(
            InsideEffect::Freeze,
            EntityWrite::SetMovement(one_way_motion(facing)),
        );
        event
            .effects
            .run_before(InsideEffect::Freeze, EntityWrite::ResetFallDistance);
    }
}

/// The devices' items: placing one records it.
struct DeviceItem;

#[mod_api::horizon_part]
impl ItemPart for DeviceItem {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        let Some(kind) = kind_of(event.state.block()) else {
            return;
        };
        let pos = event.pos;
        let chunk = Holder::Chunk([pos[0] >> 4, pos[2] >> 4]);
        let Ok(mut devices) = DEVICES.get(host.attachments(), chunk) else {
            return;
        };
        devices.retain(|d| d.pos != pos);
        devices.push(Device { pos, kind });
        let _ = DEVICES.set(host.attachments(), chunk, devices);
    }
}

/// Move the entities in front of one powered device.
fn run_device(host: &mut dyn ServerHost, level: LevelRef, device: Device) {
    let Some(state) = level.block_state(host, device.pos) else {
        return;
    };
    if kind_of(state.block()) != Some(device.kind) {
        return;
    }
    let range = level.best_neighbor_signal(host, device.pos).unwrap_or(0);
    if range <= 0 {
        return;
    }
    let Some(facing) = level
        .block_property(host, device.pos, "facing")
        .and_then(|f| Facing::from_name(&f))
    else {
        return;
    };
    let [min, max] = beam(device.pos, facing, range);
    for entity in level.entities_in(host, min, max).unwrap_or_default() {
        let velocity = entity.velocity(host).unwrap_or([0.0; 3]);
        if let Some(delta) = device_push(device.kind, facing, velocity) {
            entity.push(host, delta);
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let keys = [
            r.block_key(Kind::Attractor.id()),
            r.block_key(Kind::Pusher.id()),
            r.block_key("restrictions:one_way"),
        ];
        let _ = KEYS.set(keys);
        for kind in [Kind::Attractor, Kind::Pusher] {
            r.block_part(keys[kind.code() as usize], Placed { along_look: false });
            let item = r.item_key(kind.id());
            r.item_part(item, DeviceItem);
        }
        r.block_part(keys[2], OneWay);
    }

    fn level_tick_post(&self, host: HostSide, event: &mut LevelTickPostEvent) {
        let HostSide::Server(host) = host else {
            return;
        };
        let Ok(query) = host.query(&DEVICES) else {
            return;
        };
        let devices: Vec<Device> = query
            .iter()
            .filter_map(Result::ok)
            .flat_map(|row| row.value.to_vec())
            .collect();
        drop(query);
        for device in devices {
            run_device(host, event.level, device);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_look_picks_the_nearest_face() {
        // Vanilla: yaw 0 looks south, 90 west, 180 north, -90 east; pitch 90 looks down.
        assert_eq!(Facing::looking(0.0, 0.0), Facing::South);
        assert_eq!(Facing::looking(90.0, 10.0), Facing::West);
        assert_eq!(Facing::looking(180.0, -20.0), Facing::North);
        assert_eq!(Facing::looking(-90.0, 0.0), Facing::East);
        assert_eq!(Facing::looking(30.0, 80.0), Facing::Down);
        assert_eq!(Facing::looking(0.0, -60.0), Facing::Up);
    }

    #[test]
    fn the_beam_starts_at_the_front_face_and_reaches_the_signal() {
        assert_eq!(
            beam([10, 64, -5], Facing::East, 4),
            [[11.0, 64.0, -5.0], [15.0, 65.0, -4.0]]
        );
        assert_eq!(
            beam([0, 64, 0], Facing::Down, 15),
            [[0.0, 49.0, 0.0], [1.0, 64.0, 1.0]]
        );
    }

    #[test]
    fn devices_push_along_the_beam_until_the_cap() {
        assert_eq!(
            device_push(Kind::Pusher, Facing::North, [0.0; 3]),
            Some([0.0, 0.0, -DEVICE_ACCEL])
        );
        assert_eq!(
            device_push(Kind::Attractor, Facing::North, [0.3, 0.0, 0.1]),
            Some([0.0, 0.0, DEVICE_ACCEL])
        );
        assert_eq!(
            device_push(Kind::Attractor, Facing::Up, [0.0, -0.7, 0.0]),
            None
        );
    }

    #[test]
    fn devices_round_trip_through_their_saved_form() {
        let devices = vec![
            Device {
                pos: [3, -20, 7],
                kind: Kind::Pusher,
            },
            Device {
                pos: [-1, 300, 0],
                kind: Kind::Attractor,
            },
        ];
        assert_eq!(
            decode_devices(&encode_devices(&devices).unwrap()).unwrap(),
            devices
        );
    }

    #[test]
    fn the_one_way_moves_along_its_facing() {
        assert_eq!(one_way_motion(Facing::Up), [0.0, ONE_WAY_SPEED, 0.0]);
        assert_eq!(one_way_motion(Facing::West), [-ONE_WAY_SPEED, 0.0, 0.0]);
    }
}
