//! Eternal Starlight for Horizon: the Starlight portal.
//!
//! Written from Eternal Starlight's observed behaviour and public description, never from its
//! code (docs/modpacks.md, "Licences"). What the port does, and what a harness against the
//! NeoForge original still has to confirm, is in README.md.
//!
//! How it works: using a Blossom of Stars on a frame fills the frame's opening with
//! `eternal_starlight:starlight_portal`. The frame may be of any solid blocks and any shape: the
//! opening is the air the frame closes in, in one vertical plane. A player who stands in the
//! portal is sent to `eternal_starlight:starlight` at the same x and z; a small portal of the
//! same frame block is built beside where they land, so they can come back.
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Mutex, OnceLock};

use mod_api::{
    BlockKey, BlockState, BlockUpdateFlags, Hooks, HostBlockStates, HostSide, InteractionResult,
    ItemPart, LevelRef, PlayerChangedDimensionEvent, PlayerTickPostEvent, Registrar,
    RegistrationContext, ServerHost, UseOnEvent,
};

/// The Starlight's level.
pub const STARLIGHT: &str = "eternal_starlight:starlight";
/// Where a portal in the Starlight leads.
pub const OVERWORLD: &str = "minecraft:overworld";
/// The most blocks an opening holds.
pub const MAX_OPENING: usize = 21 * 21;
/// The widest and highest an opening may be, in blocks.
pub const MAX_SIDE: i32 = 21;
/// Ticks a survival player stands in the portal before it takes them.
pub const SURVIVAL_WAIT: u32 = 80;
/// Ticks before a player who just arrived can be taken again.
pub const COOLDOWN: i32 = 300;

/// The vertical plane an opening lies in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plane {
    /// Constant z: the opening runs along x.
    X,
    /// Constant x: the opening runs along z.
    Z,
}

impl Plane {
    fn sideways(self) -> [i32; 3] {
        match self {
            Plane::X => [1, 0, 0],
            Plane::Z => [0, 0, 1],
        }
    }
}

/// What the search sees at one block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    /// Air or a portal block: the opening may hold it.
    Open,
    /// A solid block: part of the frame.
    Frame,
    /// A fluid or a block that is neither (grass, a torch): the frame isn't closed.
    Leak,
}

/// The opening around `start` in `plane`: every open block reached from `start` without crossing
/// the frame. None when `start` isn't open, when the search touches a leak, or when the opening
/// is larger than `MAX_OPENING` blocks or `MAX_SIDE` across, which is how an unclosed frame
/// shows. Needs at least 2 blocks.
pub fn find_opening(
    start: [i32; 3],
    plane: Plane,
    at: impl Fn([i32; 3]) -> Cell,
) -> Option<Vec<[i32; 3]>> {
    if at(start) != Cell::Open {
        return None;
    }
    let s = plane.sideways();
    let steps = [[0, 1, 0], [0, -1, 0], s, [-s[0], 0, -s[2]]];
    let mut seen = HashSet::from([start]);
    let mut queue = VecDeque::from([start]);
    let mut out = Vec::new();
    let (mut lo, mut hi) = (start, start);
    while let Some(p) = queue.pop_front() {
        out.push(p);
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
        if out.len() > MAX_OPENING || (0..3).any(|i| hi[i] - lo[i] >= MAX_SIDE) {
            return None;
        }
        for d in steps {
            let n = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
            if seen.contains(&n) {
                continue;
            }
            match at(n) {
                Cell::Open => {
                    seen.insert(n);
                    queue.push_back(n);
                }
                Cell::Frame => {}
                Cell::Leak => return None,
            }
        }
    }
    (out.len() >= 2).then_some(out)
}

/// Where a portal sends a player at `pos` in `from`. None outside the two levels it joins.
pub fn destination(from: &str, pos: [f64; 3]) -> Option<(&'static str, [f64; 3])> {
    match from {
        OVERWORLD => Some((STARLIGHT, pos)),
        STARLIGHT => Some((OVERWORLD, pos)),
        _ => None,
    }
}

/// The blocks of a small return portal two blocks along z from `at`: (position, is frame).
pub fn return_portal(at: [i32; 3]) -> Vec<([i32; 3], bool)> {
    let base = [at[0] - 1, at[1] - 1, at[2] + 2];
    let mut out = Vec::new();
    for dx in 0..4 {
        for dy in 0..5 {
            let frame = dx == 0 || dx == 3 || dy == 0 || dy == 4;
            out.push(([base[0] + dx, base[1] + dy, base[2]], frame));
        }
    }
    out
}

struct Keys {
    portal: BlockKey,
    air: [BlockKey; 3],
}

static KEYS: OnceLock<Keys> = OnceLock::new();
static WAITING: Mutex<Option<HashMap<u128, u32>>> = Mutex::new(None);
/// Players sent through a portal, with where they land and the frame block they lit.
static ARRIVING: Mutex<Option<HashMap<u128, ([i32; 3], BlockState)>>> = Mutex::new(None);

fn cell(host: &dyn ServerHost, level: LevelRef, pos: [i32; 3]) -> Cell {
    let Some(keys) = KEYS.get() else {
        return Cell::Leak;
    };
    match level.block(host, pos) {
        Some(b) if b == keys.portal || keys.air.contains(&b) => Cell::Open,
        Some(_) => {
            let state = level.block_state(host, pos);
            let solid = state.is_some_and(|s| host.block_states().replaceable(s) == Some(false));
            let fluid = level.fluid_state(host, pos).is_some_and(|f| !f.is_empty());
            if solid && !fluid {
                Cell::Frame
            } else {
                Cell::Leak
            }
        }
        None => Cell::Leak,
    }
}

fn default_state(host: &dyn ServerHost, block: BlockKey) -> Option<BlockState> {
    BlockState::of(&host.block_states(), block, &[]).ok()
}

/// The Blossom of Stars: lights a frame it is used on.
struct BlossomOfStars;

#[mod_api::horizon_part]
impl ItemPart for BlossomOfStars {
    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        let keys = KEYS.get()?;
        let level = host.player(event.player)?.level(host)?;
        let start = [
            event.pos[0] + event.face[0],
            event.pos[1] + event.face[1],
            event.pos[2] + event.face[2],
        ];
        let opening = [Plane::X, Plane::Z]
            .into_iter()
            .find_map(|plane| find_opening(start, plane, |p| cell(host, level, p)))?;
        let portal = default_state(host, keys.portal)?;
        for pos in opening {
            level
                .set_block(host, pos, portal, BlockUpdateFlags::CLIENTS)
                .ok()?;
        }
        level.play_sound(
            host,
            "minecraft:block.amethyst_block.resonate",
            [
                start[0] as f64 + 0.5,
                start[1] as f64 + 0.5,
                start[2] as f64 + 0.5,
            ],
            mod_api::SoundSource::Blocks,
            1.0,
            1.0,
        );
        Some(InteractionResult::Success)
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let _ = KEYS.set(Keys {
            portal: r.block_key("eternal_starlight:starlight_portal"),
            air: [
                r.block_key("minecraft:air"),
                r.block_key("minecraft:cave_air"),
                r.block_key("minecraft:void_air"),
            ],
        });
        let blossom = r.item_key("eternal_starlight:blossom_of_stars");
        r.item_part(blossom, BlossomOfStars);
    }

    fn player_tick_post(&self, host: HostSide, event: &mut PlayerTickPostEvent) {
        let HostSide::Server(host) = host else {
            return;
        };
        let Some(keys) = KEYS.get() else {
            return;
        };
        let player = event.player;
        let entity = player.entity();
        let uuid = player.uuid();
        let feet = entity.block_position(host);
        let inside = feet.is_some_and(|feet| {
            player
                .level(host)
                .is_some_and(|level| level.block(host, feet) == Some(keys.portal))
        });
        let mut waiting = WAITING.lock().unwrap_or_else(|e| e.into_inner());
        let waiting = waiting.get_or_insert_with(HashMap::new);
        if !inside || entity.portal_cooldown(host).unwrap_or(0) > 0 {
            waiting.remove(&uuid);
            return;
        }
        let ticks = waiting.entry(uuid).or_insert(0);
        *ticks += 1;
        let wait = if player.is_creative(host).unwrap_or(false) {
            1
        } else {
            SURVIVAL_WAIT
        };
        if *ticks < wait {
            return;
        }
        waiting.remove(&uuid);
        let (Some(from), Some(pos), Some(feet)) =
            (player.dimension(host), entity.position(host), feet)
        else {
            return;
        };
        let Some((to, at)) = destination(&from.to_string(), pos) else {
            return;
        };
        // The frame block under the portal, read before the player leaves its level.
        let frame = player.level(host).and_then(|level| {
            (1..=MAX_SIDE)
                .map(|dy| [feet[0], feet[1] - dy, feet[2]])
                .find(|p| level.block(host, *p) != Some(keys.portal))
                .filter(|p| cell(host, level, *p) == Cell::Frame)
                .and_then(|p| level.block_state(host, p))
        });
        entity.set_portal_cooldown(host, COOLDOWN);
        if entity.teleport_to_level(host, to, at, None)
            && let Some(frame) = frame
        {
            let land = [
                at[0].floor() as i32,
                at[1].floor() as i32,
                at[2].floor() as i32,
            ];
            ARRIVING
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get_or_insert_with(HashMap::new)
                .insert(uuid, (land, frame));
        }
    }

    fn player_changed_dimension(
        &self,
        host: &mut dyn ServerHost,
        event: &mut PlayerChangedDimensionEvent<'_>,
    ) {
        let arrived = ARRIVING
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_or_insert_with(HashMap::new)
            .remove(&event.player);
        let Some((at, frame)) = arrived else {
            return;
        };
        let Some(keys) = KEYS.get() else {
            return;
        };
        let Some(level) = host.player(event.player).and_then(|p| p.level(host)) else {
            return;
        };
        let (Some(portal), Some(air)) = (
            default_state(host, keys.portal),
            default_state(host, keys.air[0]),
        ) else {
            return;
        };
        let near = (-2..=2).any(|dx| {
            (-1..=3).any(|dy| {
                (-2..=3).any(|dz| {
                    level.block(host, [at[0] + dx, at[1] + dy, at[2] + dz]) == Some(keys.portal)
                })
            })
        });
        if near {
            return;
        }
        for (pos, is_frame) in return_portal(at) {
            let state = if is_frame { frame } else { portal };
            let _ = level.set_block(host, pos, state, BlockUpdateFlags::CLIENTS);
        }
        for dy in 0..2 {
            let _ = level.set_block(
                host,
                [at[0], at[1] + dy, at[2]],
                air,
                BlockUpdateFlags::CLIENTS,
            );
        }
        if cell(host, level, [at[0], at[1] - 1, at[2]]) == Cell::Open {
            let _ = level.set_block(
                host,
                [at[0], at[1] - 1, at[2]],
                frame,
                BlockUpdateFlags::CLIENTS,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A world in the plane z = 0 drawn as rows from the top: `#` frame, `.` open, `~` leak;
    /// the bottom row is y = 0 and the first column x = 0. Everything off the drawing is a leak.
    fn drawn(rows: &[&str]) -> impl Fn([i32; 3]) -> Cell + use<> {
        let rows: Vec<Vec<u8>> = rows.iter().rev().map(|r| r.bytes().collect()).collect();
        move |p| {
            if p[2] != 0 || p[1] < 0 || p[0] < 0 {
                return Cell::Leak;
            }
            match rows.get(p[1] as usize).and_then(|r| r.get(p[0] as usize)) {
                Some(b'#') => Cell::Frame,
                Some(b'.') => Cell::Open,
                _ => Cell::Leak,
            }
        }
    }

    #[test]
    fn a_frame_of_any_shape_gives_the_air_it_closes() {
        let world = drawn(&[
            "  ###  ", //
            " #...# ", //
            "#.....#", //
            "#..#..#", //
            " ##### ",
        ]);
        let opening = find_opening([1, 2, 0], Plane::X, world).unwrap();
        assert_eq!(opening.len(), 3 + 5 + 4);
        assert!(!opening.contains(&[3, 1, 0]));
    }

    #[test]
    fn an_open_frame_is_refused() {
        let gap = drawn(&[
            "####", //
            "#..~", //
            "#..#", //
            "####",
        ]);
        assert_eq!(find_opening([1, 1, 0], Plane::X, gap), None);
        // A frame of the wrong plane holds nothing: the search leaks off the drawing.
        let closed = drawn(&["####", "#..#", "####"]);
        assert_eq!(find_opening([1, 1, 0], Plane::Z, closed), None);
    }

    #[test]
    fn an_opening_wider_than_the_limit_is_refused() {
        let n = MAX_SIDE as usize;
        let wide = format!("#{}#", ".".repeat(n + 1));
        let edge = "#".repeat(n + 3);
        let world = drawn(&[&edge, &wide, &edge]);
        assert_eq!(find_opening([1, 1, 0], Plane::X, world), None);
        let fits = format!("#{}#", ".".repeat(n));
        let edge = "#".repeat(n + 2);
        let world = drawn(&[&edge, &fits, &edge]);
        assert_eq!(
            find_opening([1, 1, 0], Plane::X, world).map(|o| o.len()),
            Some(n)
        );
    }

    #[test]
    fn the_portal_joins_the_overworld_and_the_starlight() {
        let pos = [0.5, 70.0, 0.5];
        assert_eq!(destination(OVERWORLD, pos), Some((STARLIGHT, pos)));
        assert_eq!(destination(STARLIGHT, pos), Some((OVERWORLD, pos)));
        assert_eq!(destination("minecraft:the_end", pos), None);
    }
}
