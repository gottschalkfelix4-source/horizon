//! Deeper and Darker for Horizon: the Otherside portal.
//!
//! Written from Deeper and Darker's observed behaviour and public description, never from its
//! code (docs/modpacks.md, "Licences"). What the port does, and what a harness against the
//! NeoForge original still has to confirm, is in README.md.
//!
//! How it works: using the Heart of the Deep on a reinforced deepslate frame (the one at the
//! heart of every ancient city, or one a player builds) finds the frame's rectangular opening
//! and fills it with `deeperdarker:otherside_portal`. A player who stands in that portal long
//! enough is sent to `deeperdarker:otherside` at the same x and z, and a small reinforced
//! deepslate portal is built beside where they arrive so they can come back. Standing in a
//! portal in the Otherside sends them to the Overworld the same way.
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use mod_api::{
    BlockKey, BlockState, BlockUpdateFlags, Hooks, HostBlockStates, HostSide, InteractionResult,
    ItemPart, PlayerChangedDimensionEvent, PlayerTickPostEvent, Registrar, RegistrationContext,
    ServerHost, UseOnEvent,
};

/// The Otherside's level.
pub const OTHERSIDE: &str = "deeperdarker:otherside";
/// Where a portal in the Otherside leads.
pub const OVERWORLD: &str = "minecraft:overworld";
/// Ticks a survival player stands in the portal before it takes them (the nether portal's wait).
pub const SURVIVAL_WAIT: u32 = 80;
/// Ticks before a player who just arrived can be taken again (vanilla's portal cooldown).
pub const COOLDOWN: i32 = 300;
/// The largest opening on either side, in blocks. The ancient city's frame is the largest one a
/// player meets in survival; this is set above it.
pub const MAX_SIDE: i32 = 32;

/// The axis a vertical portal's opening runs along.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Z,
}

impl Axis {
    fn step(self) -> [i32; 3] {
        match self {
            Axis::X => [1, 0, 0],
            Axis::Z => [0, 0, 1],
        }
    }
}

/// What a frame search sees at one block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    /// Reinforced deepslate.
    Frame,
    /// Air, or a portal block already there: the opening may hold it.
    Empty,
    /// Anything else.
    Other,
}

/// A frame's opening: its lowest corner, the axis it runs along, its width and height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Opening {
    pub corner: [i32; 3],
    pub axis: Axis,
    pub width: i32,
    pub height: i32,
}

impl Opening {
    /// Every block of the opening, bottom row first.
    pub fn blocks(&self) -> Vec<[i32; 3]> {
        let s = self.axis.step();
        let mut out = Vec::with_capacity((self.width * self.height) as usize);
        for h in 0..self.height {
            for w in 0..self.width {
                out.push([
                    self.corner[0] + s[0] * w,
                    self.corner[1] + h,
                    self.corner[2] + s[2] * w,
                ]);
            }
        }
        out
    }
}

fn add(a: [i32; 3], b: [i32; 3], k: i32) -> [i32; 3] {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

/// The rectangular opening of a frame along `axis` that holds `start`, if `start` is empty and the
/// frame closes all around it: as a nether portal's shape, every block of the opening empty, the
/// row below and above it and the columns on both sides all frame (corners not needed). At least
/// 1 wide and 2 high, at most `MAX_SIDE` each way.
pub fn find_opening(start: [i32; 3], axis: Axis, at: impl Fn([i32; 3]) -> Cell) -> Option<Opening> {
    if at(start) != Cell::Empty {
        return None;
    }
    let s = axis.step();
    // Down to the floor.
    let mut bottom = start;
    for _ in 0..MAX_SIDE {
        if at(add(bottom, [0, -1, 0], 1)) != Cell::Empty {
            break;
        }
        bottom = add(bottom, [0, -1, 0], 1);
    }
    if at(add(bottom, [0, -1, 0], 1)) != Cell::Frame {
        return None;
    }
    // Back to the left side.
    let mut corner = bottom;
    for _ in 0..MAX_SIDE {
        let next = add(corner, s, -1);
        if at(next) != Cell::Empty {
            break;
        }
        corner = next;
    }
    if at(add(corner, s, -1)) != Cell::Frame {
        return None;
    }
    // Width: empty along the bottom with frame under each block, ending at frame.
    let mut width = 0;
    while width < MAX_SIDE {
        let p = add(corner, s, width);
        if at(p) != Cell::Empty || at(add(p, [0, -1, 0], 1)) != Cell::Frame {
            break;
        }
        width += 1;
    }
    if width == 0 || at(add(corner, s, width)) != Cell::Frame {
        return None;
    }
    // Height: whole rows empty with frame at both ends, closed by a row of frame.
    let mut height = 0;
    loop {
        if height >= MAX_SIDE {
            return None;
        }
        let row = add(corner, [0, 1, 0], height);
        let top = (0..width).all(|w| at(add(row, s, w)) == Cell::Frame);
        if top {
            break;
        }
        let open = (0..width).all(|w| at(add(row, s, w)) == Cell::Empty)
            && at(add(row, s, -1)) == Cell::Frame
            && at(add(row, s, width)) == Cell::Frame;
        if !open {
            return None;
        }
        height += 1;
    }
    (height >= 2).then_some(Opening {
        corner,
        axis,
        width,
        height,
    })
}

/// Where a portal sends a player standing at `pos` in `from`: the other level, the same x and z.
/// None outside the two levels the portal joins.
pub fn destination(from: &str, pos: [f64; 3]) -> Option<(&'static str, [f64; 3])> {
    match from {
        OVERWORLD => Some((OTHERSIDE, pos)),
        OTHERSIDE => Some((OVERWORLD, pos)),
        _ => None,
    }
}

/// How long a player waits in a portal. Creative players go at once, as through a nether portal.
pub fn wait_ticks(creative: bool) -> u32 {
    if creative { 1 } else { SURVIVAL_WAIT }
}

/// Ids resolved at registration.
struct Keys {
    portal: BlockKey,
    frame: BlockKey,
    air: [BlockKey; 3],
}

static KEYS: OnceLock<Keys> = OnceLock::new();

/// Ticks each player has stood in a portal; a player who steps out starts again.
static WAITING: Mutex<Option<HashMap<u128, u32>>> = Mutex::new(None);

/// Players this mod just sent through a portal: they get a return portal where they land.
static ARRIVING: Mutex<Option<HashMap<u128, [i32; 3]>>> = Mutex::new(None);

fn cell(host: &dyn ServerHost, level: mod_api::LevelRef, pos: [i32; 3]) -> Cell {
    let Some(keys) = KEYS.get() else {
        return Cell::Other;
    };
    match level.block(host, pos) {
        Some(b) if b == keys.frame => Cell::Frame,
        Some(b) if b == keys.portal || keys.air.contains(&b) => Cell::Empty,
        _ => Cell::Other,
    }
}

fn default_state(host: &dyn ServerHost, block: BlockKey) -> Option<BlockState> {
    BlockState::of(&host.block_states(), block, &[]).ok()
}

/// Fill `opening` with portal blocks.
fn light(host: &mut dyn ServerHost, level: mod_api::LevelRef, opening: &Opening) -> bool {
    let Some(keys) = KEYS.get() else {
        return false;
    };
    let Some(portal) = default_state(host, keys.portal) else {
        return false;
    };
    for pos in opening.blocks() {
        if level
            .set_block(host, pos, portal, BlockUpdateFlags::CLIENTS)
            .is_err()
        {
            return false;
        }
    }
    true
}

/// The Heart of the Deep: lights a frame it is used on.
struct HeartOfTheDeep;

#[mod_api::horizon_part]
impl ItemPart for HeartOfTheDeep {
    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        let player = host.player(event.player)?;
        let level = player.level(host)?;
        let start = add(event.pos, event.face, 1);
        let opening = [Axis::X, Axis::Z]
            .into_iter()
            .find_map(|axis| find_opening(start, axis, |p| cell(host, level, p)))?;
        if !light(host, level, &opening) {
            return None;
        }
        level.play_sound(
            host,
            "minecraft:block.sculk_shrieker.shriek",
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
            portal: r.block_key("deeperdarker:otherside_portal"),
            frame: r.block_key("minecraft:reinforced_deepslate"),
            air: [
                r.block_key("minecraft:air"),
                r.block_key("minecraft:cave_air"),
                r.block_key("minecraft:void_air"),
            ],
        });
        let heart = r.item_key("deeperdarker:heart_of_the_deep");
        r.item_part(heart, HeartOfTheDeep);
    }

    /// Count the ticks a player stands in a portal, then send them through.
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
        let inside = (|| {
            let level = player.level(host)?;
            let feet = entity.block_position(host)?;
            Some(level.block(host, feet) == Some(keys.portal))
        })()
        .unwrap_or(false);
        let mut waiting = WAITING.lock().unwrap_or_else(|e| e.into_inner());
        let waiting = waiting.get_or_insert_with(HashMap::new);
        if !inside || entity.portal_cooldown(host).unwrap_or(0) > 0 {
            waiting.remove(&uuid);
            return;
        }
        let ticks = waiting.entry(uuid).or_insert(0);
        *ticks += 1;
        let creative = player.is_creative(host).unwrap_or(false);
        if *ticks < wait_ticks(creative) {
            return;
        }
        waiting.remove(&uuid);
        let Some(from) = player.dimension(host) else {
            return;
        };
        let Some(pos) = entity.position(host) else {
            return;
        };
        let Some((to, at)) = destination(&from.to_string(), pos) else {
            return;
        };
        entity.set_portal_cooldown(host, COOLDOWN);
        if entity.teleport_to_level(host, to, at, None) {
            ARRIVING
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get_or_insert_with(HashMap::new)
                .insert(
                    uuid,
                    [
                        at[0].floor() as i32,
                        at[1].floor() as i32,
                        at[2].floor() as i32,
                    ],
                );
        }
    }

    /// A player this mod sent through arrives: build a way back beside them.
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
        let Some(at) = arrived else {
            return;
        };
        let Some(level) = host.player(event.player).and_then(|p| p.level(host)) else {
            return;
        };
        build_return_portal(host, level, at);
    }
}

/// The blocks of a small return portal standing two blocks along z from `at`, with room to
/// stand in front of it: (position, is frame). The opening is 2 wide and 3 high.
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

fn build_return_portal(host: &mut dyn ServerHost, level: mod_api::LevelRef, at: [i32; 3]) {
    let Some(keys) = KEYS.get() else {
        return;
    };
    let (Some(frame), Some(portal), Some(air)) = (
        default_state(host, keys.frame),
        default_state(host, keys.portal),
        default_state(host, keys.air[0]),
    ) else {
        return;
    };
    // Already a portal next to the player: a way back exists.
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
    // Room to stand: clear the two blocks above the arrival point and give it a floor.
    for dy in 0..2 {
        let _ = level.set_block(
            host,
            [at[0], at[1] + dy, at[2]],
            air,
            BlockUpdateFlags::CLIENTS,
        );
    }
    if cell(host, level, [at[0], at[1] - 1, at[2]]) == Cell::Empty {
        let _ = level.set_block(
            host,
            [at[0], at[1] - 1, at[2]],
            frame,
            BlockUpdateFlags::CLIENTS,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// A frame of `w`x`h` opening along x with its lowest opening block at the origin.
    fn frame(w: i32, h: i32) -> HashSet<[i32; 3]> {
        let mut set = HashSet::new();
        for x in -1..=w {
            set.insert([x, -1, 0]);
            set.insert([x, h, 0]);
        }
        for y in 0..h {
            set.insert([-1, y, 0]);
            set.insert([w, y, 0]);
        }
        set
    }

    fn world(frame: &HashSet<[i32; 3]>) -> impl Fn([i32; 3]) -> Cell + '_ {
        move |p| {
            if frame.contains(&p) {
                Cell::Frame
            } else if p[2] == 0 && (-1..=40).contains(&p[0]) && (-1..=40).contains(&p[1]) {
                Cell::Empty
            } else {
                Cell::Other
            }
        }
    }

    #[test]
    fn a_closed_frame_gives_its_whole_opening_from_any_block_in_it() {
        let f = frame(5, 4);
        let found = find_opening([3, 2, 0], Axis::X, world(&f)).unwrap();
        assert_eq!(
            found,
            Opening {
                corner: [0, 0, 0],
                axis: Axis::X,
                width: 5,
                height: 4
            }
        );
        assert_eq!(found.blocks().len(), 20);
        // The frame runs along x, so it is not found along z.
        assert_eq!(find_opening([3, 2, 0], Axis::Z, world(&f)), None);
    }

    #[test]
    fn a_frame_with_a_gap_or_a_low_opening_is_refused() {
        let mut f = frame(3, 3);
        f.remove(&[3, 1, 0]);
        assert_eq!(find_opening([1, 1, 0], Axis::X, world(&f)), None);
        let flat = frame(4, 1);
        assert_eq!(find_opening([1, 0, 0], Axis::X, world(&flat)), None);
    }

    #[test]
    fn the_portal_joins_the_overworld_and_the_otherside_only() {
        let pos = [10.5, 64.0, -3.5];
        assert_eq!(destination(OVERWORLD, pos), Some((OTHERSIDE, pos)));
        assert_eq!(destination(OTHERSIDE, pos), Some((OVERWORLD, pos)));
        assert_eq!(destination("minecraft:the_nether", pos), None);
        assert_eq!(wait_ticks(false), 80);
        assert_eq!(wait_ticks(true), 1);
    }

    #[test]
    fn the_return_portal_is_a_closed_frame_with_a_two_by_three_opening() {
        let blocks = return_portal([0, 64, 0]);
        let frame: HashSet<[i32; 3]> = blocks.iter().filter(|b| b.1).map(|b| b.0).collect();
        let opening: Vec<[i32; 3]> = blocks.iter().filter(|b| !b.1).map(|b| b.0).collect();
        assert_eq!(opening.len(), 6);
        let at = |p: [i32; 3]| {
            if frame.contains(&p) {
                Cell::Frame
            } else if opening.contains(&p) {
                Cell::Empty
            } else {
                Cell::Other
            }
        };
        let found = find_opening(opening[0], Axis::X, at).unwrap();
        assert_eq!((found.width, found.height), (2, 3));
        // The arrival point stays outside the portal.
        assert!(!opening.contains(&[0, 64, 0]));
    }
}
