//! RFTools Builder for Horizon: the Builder, given a Quarry, Clear Quarry or Void shape card,
//! digs out the card's shape block by block, paying RF from its own buffer, and puts what it
//! mines in the inventory above it.
//!
//! Written from RFTools Builder's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What is ported and what still has to be checked against the
//! NeoForge original is in README.md.
//!
//! How it works: the Builder is a block with a block entity and an energy buffer the game keeps
//! (`Registrar::block_energy`). Its card (shape, size, offset, mode) and its progress are kept per
//! level in the attachment `rftoolsbuilder:builders`, set with `/rftoolsbuilder card` until the
//! card items and the Builder's screen exist. After each level tick, every Builder with a card
//! and the RF for one block goes through its shape's blocks top layer first and takes the next
//! one it can: a Quarry card leaves dirt behind, a Clear Quarry card air, a Void card destroys the
//! block without drops.
use std::sync::OnceLock;

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockKey, BlockState, BlockUpdateFlags,
    CommandArgumentValue, Compound, Direction, ENERGY, Flow, Holder, Hooks, HostBlockStates,
    HostSide, HostTransaction, LevelRef, LevelTickPostEvent, ModEnergyBuffer,
    RegisterCommandsEvent, Registrar, RegistrationContext, RunModCommandEvent, STORAGE, ServerHost,
    Tag, Text,
};

pub mod shape;

use shape::{Area, Shape};

/// RF one block costs (all three cards). From memory: to check against the original's config.
pub const RF_PER_BLOCK: u32 = 300;
/// The Builder's buffer. From memory: to check against the original's config.
pub const CAPACITY: i32 = 500_000;
/// The most RF one insert puts in.
pub const MAX_RECEIVE: i32 = 20_000;
/// Blocks of the box looked at per tick before the Builder waits for the next one, so a large
/// empty area costs a bounded amount per tick.
pub const SCAN_PER_TICK: u32 = 256;
/// The largest side of a card's box.
pub const MAX_SIDE: i32 = 256;

/// What the Builder does with each block of the shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Mines it into the inventory above and leaves dirt (less lighting work than air).
    Quarry,
    /// Mines it into the inventory above and leaves air.
    ClearQuarry,
    /// Destroys it, without drops.
    Void,
}

impl Mode {
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "quarry" => Mode::Quarry,
            "clearquarry" => Mode::ClearQuarry,
            "void" => Mode::Void,
            _ => return None,
        })
    }
    fn from_code(code: i32) -> Option<Self> {
        [Mode::Quarry, Mode::ClearQuarry, Mode::Void]
            .get(usize::try_from(code).ok()?)
            .copied()
    }
}

/// One Builder with a card.
#[derive(Clone, Debug, PartialEq)]
pub struct Builder {
    pub pos: [i32; 3],
    pub mode: Mode,
    pub area: Area,
    /// The next block of the area's box to look at.
    pub cursor: u64,
}

impl Builder {
    /// The next block to work on from the cursor, looking at no more than `budget` blocks:
    /// `Ok(pos)` to work on, `Err(true)` when the area is done, `Err(false)` to look further next
    /// tick. `workable` says whether a block is one the card takes (not air, not unbreakable).
    pub fn next_target(
        &mut self,
        budget: u32,
        workable: &mut dyn FnMut([i32; 3]) -> bool,
    ) -> Result<[i32; 3], bool> {
        for _ in 0..budget {
            let Some(pos) = self.area.nth(self.cursor) else {
                return Err(true);
            };
            self.cursor += 1;
            if pos != self.pos && self.area.contains(pos) && workable(pos) {
                return Ok(pos);
            }
        }
        Err(false)
    }
}

static BUILDERS: AttachmentType<Vec<Builder>> = AttachmentType {
    namespace: "rftoolsbuilder",
    name: "builders",
    default: Vec::new,
    codec: Some(AttachmentCodec {
        encode: encode_builders,
        decode: decode_builders,
    }),
    copy_on_death: false,
    sync: false,
};

/// Saved as `{builders: [I; x, y, z, mode, shape, solid, w, h, l, cx, cy, cz, cursor_hi, cursor_lo, ...]}`.
const FIELDS: usize = 14;

fn encode_builders(builders: &Vec<Builder>) -> Result<Compound, AttachmentError> {
    let flat = builders
        .iter()
        .flat_map(|b| {
            let a = b.area;
            [
                b.pos[0],
                b.pos[1],
                b.pos[2],
                b.mode as i32,
                a.shape.code(),
                i32::from(a.solid),
                a.dim[0],
                a.dim[1],
                a.dim[2],
                a.center[0],
                a.center[1],
                a.center[2],
                (b.cursor >> 32) as i32,
                b.cursor as u32 as i32,
            ]
        })
        .collect();
    Ok([("builders".into(), Tag::IntArray(flat))].into())
}

fn decode_builders(data: &Compound) -> Result<Vec<Builder>, AttachmentError> {
    let Some(Tag::IntArray(flat)) = data.get(&"builders".into()) else {
        return Err(AttachmentError::Decode);
    };
    if flat.len() % FIELDS != 0 {
        return Err(AttachmentError::Decode);
    }
    Ok(flat
        .chunks_exact(FIELDS)
        .filter_map(|c| {
            Some(Builder {
                pos: [c[0], c[1], c[2]],
                mode: Mode::from_code(c[3])?,
                area: Area {
                    shape: Shape::from_code(c[4])?,
                    solid: c[5] != 0,
                    dim: [c[6], c[7], c[8]],
                    center: [c[9], c[10], c[11]],
                },
                cursor: (u64::from(c[12] as u32) << 32) | u64::from(c[13] as u32),
            })
        })
        .collect())
}

/// Blocks the mod reads, known once registration ran.
struct Keys {
    builder: BlockKey,
    dirt: BlockKey,
    /// Blocks the quarry passes over: air of each kind, water, lava, bedrock.
    skipped: Vec<BlockKey>,
}

static KEYS: OnceLock<Keys> = OnceLock::new();

/// Whether the quarry takes the block at `pos`: loaded, not skipped, no block entity (its
/// contents would be lost), and breakable at all.
fn workable(host: &dyn ServerHost, level: LevelRef, keys: &Keys, pos: [i32; 3]) -> bool {
    let Some(block) = level.block(host, pos) else {
        return false;
    };
    !keys.skipped.contains(&block)
        && block != keys.builder
        && host.block_entity(level, pos).is_none()
        && level
            .block_break_speed(host, pos, None)
            .is_some_and(|speed| speed > 0.0)
}

/// One tick of one Builder: true when it is done with its area.
fn tick_builder(host: &mut dyn ServerHost, level: LevelRef, keys: &Keys, b: &mut Builder) -> bool {
    let Some(energy) = level.capability(host, &ENERGY, b.pos, None) else {
        return false;
    };
    if energy.amount(host) < RF_PER_BLOCK {
        return false;
    }
    let target = match b.next_target(SCAN_PER_TICK, &mut |pos| workable(host, level, keys, pos)) {
        Ok(pos) => pos,
        Err(done) => return done,
    };
    let paid = host.transaction(|host, tx| {
        if energy.extract(host, tx, RF_PER_BLOCK) == RF_PER_BLOCK {
            Ok(())
        } else {
            Err(())
        }
    });
    if paid.is_err() {
        b.cursor -= 1;
        return false;
    }
    let drops = b.mode != Mode::Void;
    let Ok(Some(stacks)) = level.destroy_block(host, target, drops, None, true) else {
        return false;
    };
    if b.mode == Mode::Quarry
        && let Ok(dirt) = BlockState::of(&host.block_states(), keys.dirt, &[])
    {
        let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
        let _ = level.set_block(host, target, dirt, flags);
    }
    store(host, level, b.pos, stacks);
    false
}

/// Puts mined stacks in the inventory above the Builder; what doesn't fit drops on top of it.
fn store(
    host: &mut dyn ServerHost,
    level: LevelRef,
    at: [i32; 3],
    stacks: Vec<mod_api::ItemStack>,
) {
    let above = [at[0], at[1] + 1, at[2]];
    let storage = level.capability(host, &STORAGE, above, Some(Direction::Down));
    for stack in stacks {
        let count = u64::from(stack.count());
        let went = storage.as_ref().map_or(0, |storage| {
            let kind = mod_api::ItemResource::of(&stack);
            host.transaction(|host, tx| -> Result<u64, ()> {
                Ok(storage.insert(host, tx, &kind, count))
            })
            .unwrap_or(0)
        });
        if went < count {
            let mut rest = stack;
            if rest.set_count((count - went) as u32).is_ok() {
                let _ = level.pop_resource(host, above, rest);
            }
        }
    }
}

#[derive(Default)]
struct Mod;

fn int(event: &RunModCommandEvent<'_>, name: &str) -> Option<i32> {
    match event.command.argument(name)? {
        CommandArgumentValue::Integer(i) => Some(*i),
        _ => None,
    }
}

fn word<'a>(event: &'a RunModCommandEvent<'_>, name: &str) -> Option<&'a str> {
    match event.command.argument(name)? {
        CommandArgumentValue::String(s) => Some(s),
        _ => None,
    }
}

/// The card `/rftoolsbuilder card` describes, for the Builder at `pos`.
fn card(event: &RunModCommandEvent<'_>, pos: [i32; 3]) -> Option<Builder> {
    let mode = Mode::from_name(word(event, "card")?)?;
    let shape = Shape::from_name(word(event, "shape")?)?;
    let solid = matches!(
        event.command.argument("solid"),
        Some(CommandArgumentValue::Boolean(true))
    );
    let dim = [
        int(event, "width")?,
        int(event, "height")?,
        int(event, "length")?,
    ];
    if dim.iter().any(|d| !(1..=MAX_SIDE).contains(d)) {
        return None;
    }
    let offset = [int(event, "dx")?, int(event, "dy")?, int(event, "dz")?];
    Some(Builder {
        pos,
        mode,
        area: Area {
            shape,
            solid,
            dim,
            center: std::array::from_fn(|i| pos[i] + offset[i]),
        },
        cursor: 0,
    })
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        r.block_entity_type("builder")
            .blocks(&["rftoolsbuilder:builder"])
            .build();
        let builder = r.block_key("rftoolsbuilder:builder");
        r.block_energy(
            &builder,
            ModEnergyBuffer {
                capacity: CAPACITY,
                max_insert: MAX_RECEIVE,
                // Only the Builder itself draws (through no side); cables can't drain it.
                max_extract: RF_PER_BLOCK as i32,
                insert_faces: [true; 6],
                extract_faces: [false; 6],
            },
        );
        let skipped = [
            "minecraft:air",
            "minecraft:cave_air",
            "minecraft:void_air",
            "minecraft:water",
            "minecraft:lava",
            "minecraft:bedrock",
        ]
        .map(|id| r.block_key(id))
        .to_vec();
        let _ = KEYS.set(Keys {
            builder,
            dirt: r.block_key("minecraft:dirt"),
            skipped,
        });
    }

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.extend([
            mod_api::Command::new(
                "rftoolsbuilder card <builder:block_pos> <card:word> <shape:word> <solid:boolean> \
                 <width:integer> <height:integer> <length:integer> <dx:integer> <dy:integer> \
                 <dz:integer>",
            ),
            mod_api::Command::new("rftoolsbuilder stop <builder:block_pos>"),
        ]);
    }

    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        if event.command.literals.first().map(String::as_str) != Some("rftoolsbuilder") {
            return Flow::Continue;
        }
        let level = event
            .command
            .dimension
            .id(&*host)
            .and_then(|id| host.level(&id));
        let pos = match event.command.argument("builder") {
            Some(CommandArgumentValue::BlockPos(pos)) => *pos,
            _ => return Flow::Cancel,
        };
        let is_builder = level
            .zip(KEYS.get())
            .is_some_and(|(level, keys)| level.block(&*host, pos) == Some(keys.builder));
        let Ok(mut builders) = BUILDERS.get(host.attachments(), Holder::Level) else {
            return Flow::Cancel;
        };
        let (key, args) = if !is_builder {
            ("rftoolsbuilder.horizon.command.no_builder", Vec::new())
        } else if event.command.literals.get(1).map(String::as_str) == Some("stop") {
            builders.retain(|b| b.pos != pos);
            ("rftoolsbuilder.horizon.command.stopped", Vec::new())
        } else if let Some(builder) = card(event, pos) {
            let volume = builder.area.volume();
            builders.retain(|b| b.pos != pos);
            builders.push(builder);
            (
                "rftoolsbuilder.horizon.command.card_set",
                vec![Text::literal(volume.to_string())],
            )
        } else {
            ("rftoolsbuilder.horizon.command.bad_card", Vec::new())
        };
        if is_builder {
            let _ = BUILDERS.set(host.attachments(), Holder::Level, builders);
        }
        let _ = host.commands().feedback(Text::translate(key, args));
        Flow::Cancel
    }

    fn level_tick_post(&self, host: HostSide, event: &mut LevelTickPostEvent) {
        let HostSide::Server(host) = host else {
            return;
        };
        let Some(keys) = KEYS.get() else {
            return;
        };
        let Ok(mut builders) = BUILDERS.get(host.attachments(), Holder::Level) else {
            return;
        };
        if builders.is_empty() {
            return;
        }
        let level = event.level;
        // A Builder whose block has gone, or that finished its area, drops its card.
        let mut changed = false;
        builders.retain_mut(|b| {
            let keep = match level.block(&*host, b.pos) {
                None => true,
                Some(block) if block != keys.builder => false,
                Some(_) => {
                    let before = b.cursor;
                    let done = tick_builder(host, level, keys, b);
                    changed |= b.cursor != before;
                    !done
                }
            };
            changed |= !keep;
            keep
        });
        if changed {
            let _ = BUILDERS.set(host.attachments(), Holder::Level, builders);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(shape: Shape, solid: bool, dim: [i32; 3]) -> Area {
        Area {
            shape,
            solid,
            dim,
            center: [0, 0, 0],
        }
    }

    fn count(a: &Area) -> usize {
        (0..a.volume())
            .filter_map(|i| a.nth(i))
            .filter(|p| a.contains(*p))
            .count()
    }

    #[test]
    fn a_hollow_box_is_its_shell() {
        // 5x5x5 holds 125 blocks; its 3x3x3 core is not the shell.
        assert_eq!(count(&area(Shape::Box, true, [5, 5, 5])), 125);
        assert_eq!(count(&area(Shape::Box, false, [5, 5, 5])), 125 - 27);
    }

    #[test]
    fn a_sphere_holds_its_centre_and_axes_but_not_its_corners() {
        let sphere = area(Shape::Sphere, true, [9, 9, 9]);
        assert!(sphere.contains([0, 0, 0]));
        assert!(sphere.contains([4, 0, 0]));
        assert!(sphere.contains([0, -4, 0]));
        assert!(!sphere.contains([4, 4, 4]));
        assert!(!sphere.contains([3, 3, 3]));
        assert!(!sphere.contains([5, 0, 0]));
    }

    #[test]
    fn the_quarry_goes_top_layer_first() {
        let a = area(Shape::Box, true, [2, 3, 2]);
        // The box runs from -1 to 0 on x and z, -1 to 1 on y.
        assert_eq!(a.nth(0), Some([-1, 1, -1]));
        assert_eq!(a.nth(1), Some([0, 1, -1]));
        assert_eq!(a.nth(2), Some([-1, 1, 0]));
        assert_eq!(a.nth(4), Some([-1, 0, -1]));
        assert_eq!(a.nth(11), Some([0, -1, 0]));
        assert_eq!(a.nth(12), None);
    }

    #[test]
    fn the_builder_skips_itself_and_unworkable_blocks_and_finishes() {
        let mut b = Builder {
            pos: [0, 1, 0],
            mode: Mode::Quarry,
            area: Area {
                shape: Shape::Box,
                solid: true,
                dim: [1, 3, 1],
                center: [0, 0, 0],
            },
            cursor: 0,
        };
        // y=1 is the Builder, y=0 is air, y=-1 is stone.
        let mut stone = |p: [i32; 3]| p[1] == -1;
        assert_eq!(b.next_target(10, &mut stone), Ok([0, -1, 0]));
        assert_eq!(b.next_target(10, &mut stone), Err(true));
    }

    #[test]
    fn builders_round_trip_through_their_saved_form() {
        let builders = vec![Builder {
            pos: [10, -60, -3],
            mode: Mode::Void,
            area: area(Shape::BottomDome, false, [7, 4, 9]),
            cursor: (1 << 33) + 5,
        }];
        let saved = encode_builders(&builders).unwrap();
        assert_eq!(decode_builders(&saved).unwrap(), builders);
    }
}
