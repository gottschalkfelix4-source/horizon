//! ElevatorMod for Horizon: stand on an elevator block and jump to ride up to the next elevator
//! in the same column, sneak to ride down.
//!
//! Written from ElevatorMod's public description and observed behaviour, never from its code
//! (docs/modpacks.md, "Licences"). What a harness against the NeoForge original still has to
//! confirm is in README.md.
//!
//! How it works: the server hears a player jump (`player_jump`) or start sneaking
//! (`player_toggle_sneak`). When the block under their feet is an elevator, the column above
//! (or below) is searched for the nearest elevator with room for a player on top of it, within
//! [`RANGE`] blocks, and the player is teleported onto its centre.
use std::sync::OnceLock;

use mod_api::{
    BlockKey, Hooks, Host, LevelRef, PlayerJumpEvent, PlayerRef, PlayerToggleSneakEvent, Registrar,
    RegistrationContext, ServerHost, SoundSource,
};

/// The sixteen dye colours, in vanilla's order: one elevator block per colour.
pub const COLORS: [&str; 16] = [
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "light_gray",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
];

/// How far an elevator reaches, in blocks along the column (the original's default range).
pub const RANGE: i32 = 384;

/// Whether only an elevator of the same colour is a destination (the original's `sameColor`
/// option, off by default).
pub const SAME_COLOR: bool = false;

/// The sound played to the rider on arrival. Assumed: the original ships its own sound.
const ARRIVE_SOUND: &str = "minecraft:entity.enderman.teleport";

/// Which way a ride goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
}

/// What stands at one height of the column, as the search sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Probe {
    /// An elevator of colour `color` (an index into [`COLORS`]); `clear` when a player fits on
    /// top of it.
    Elevator { color: usize, clear: bool },
    /// Anything else.
    Other,
}

/// The height of the elevator a ride from the elevator at `from_y` (colour `color`) ends on:
/// the nearest elevator in direction `dir`, within `range` blocks and inside `bounds`
/// (lowest and highest block height, inclusive), with room on top. An obstructed elevator is
/// passed over. With `same_color`, elevators of other colours are passed over too.
pub fn find_destination(
    from_y: i32,
    color: usize,
    dir: Dir,
    range: i32,
    same_color: bool,
    bounds: (i32, i32),
    mut probe: impl FnMut(i32) -> Probe,
) -> Option<i32> {
    let step = match dir {
        Dir::Up => 1,
        Dir::Down => -1,
    };
    (1..=range)
        .map(|d| from_y + d * step)
        .take_while(|y| (bounds.0..=bounds.1).contains(y))
        .find(|&y| match probe(y) {
            Probe::Elevator { color: c, clear } => clear && (!same_color || c == color),
            Probe::Other => false,
        })
}

/// The space a player needs on top of the elevator at `pos`, as an AABB.
pub fn standing_room(pos: [i32; 3]) -> [f64; 6] {
    let [x, y, z] = pos.map(f64::from);
    [x + 0.2, y + 1.0, z + 0.2, x + 0.8, y + 2.8, z + 0.8]
}

/// The sixteen elevator blocks' keys, in [`COLORS`] order, known once registration ran.
static KEYS: OnceLock<Vec<BlockKey>> = OnceLock::new();

fn color_of(block: Option<BlockKey>) -> Option<usize> {
    let block = block?;
    KEYS.get()?.iter().position(|k| *k == block)
}

fn probe(host: &dyn Host, level: LevelRef, x: i32, y: i32, z: i32) -> Probe {
    match color_of(level.block(host, [x, y, z])) {
        Some(color) => Probe::Elevator {
            color,
            clear: level.has_collisions(host, standing_room([x, y, z])) == Some(false),
        },
        None => Probe::Other,
    }
}

/// Ride `player` from the elevator under them, if there is one, in direction `dir`.
fn ride(host: &mut dyn ServerHost, uuid: u128, dir: Dir) {
    let Some(player) = host.player(uuid) else {
        return;
    };
    let read: &dyn Host = &*host;
    let Some(level) = player.level(read) else {
        return;
    };
    let Some([x, y, z]) = player.entity().on_block(read) else {
        return;
    };
    let Some(color) = color_of(level.block(read, [x, y, z])) else {
        return;
    };
    let bounds = match level.height(read) {
        Some((min, height)) => (min, min + height - 1),
        None => return,
    };
    let target = find_destination(y, color, dir, RANGE, SAME_COLOR, bounds, |ty| {
        probe(read, level, x, ty, z)
    });
    if let Some(ty) = target {
        arrive(host, player, [x, ty, z]);
    }
}

fn arrive(host: &mut dyn ServerHost, player: PlayerRef, elevator: [i32; 3]) {
    let [x, y, z] = elevator.map(f64::from);
    if player
        .entity()
        .teleport_to(host, [x + 0.5, y + 1.0, z + 0.5], None)
    {
        player.play_sound(host, ARRIVE_SOUND, SoundSource::Blocks, 1.0, 1.0);
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let keys = COLORS
            .iter()
            .map(|c| r.block_key(&format!("elevatorid:elevator_{c}")))
            .collect();
        let _ = KEYS.set(keys);
    }

    /// Jumping on an elevator rides up.
    fn player_jump(&self, host: &mut dyn ServerHost, event: &mut PlayerJumpEvent<'_>) {
        ride(host, event.player, Dir::Up);
    }

    /// Starting to sneak on an elevator rides down.
    fn player_toggle_sneak(
        &self,
        host: &mut dyn ServerHost,
        event: &mut PlayerToggleSneakEvent<'_>,
    ) {
        if event.sneaking {
            ride(host, event.player, Dir::Down);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A column with elevators at the given heights (all colour 0 unless listed), the rest air.
    fn column(elevators: &[(i32, usize, bool)]) -> impl FnMut(i32) -> Probe + '_ {
        move |y| {
            elevators
                .iter()
                .find(|e| e.0 == y)
                .map_or(Probe::Other, |&(_, color, clear)| Probe::Elevator {
                    color,
                    clear,
                })
        }
    }

    #[test]
    fn rides_to_the_nearest_elevator_in_each_direction() {
        let col = [(70, 0, true), (80, 0, true), (50, 0, true), (40, 0, true)];
        let up = find_destination(64, 0, Dir::Up, RANGE, false, (-64, 319), column(&col));
        let down = find_destination(64, 0, Dir::Down, RANGE, false, (-64, 319), column(&col));
        assert_eq!(up, Some(70));
        assert_eq!(down, Some(50));
    }

    #[test]
    fn passes_over_an_obstructed_elevator() {
        let col = [(70, 0, false), (75, 3, true)];
        let up = find_destination(64, 0, Dir::Up, RANGE, false, (-64, 319), column(&col));
        assert_eq!(up, Some(75));
    }

    #[test]
    fn same_color_skips_other_colours() {
        let col = [(70, 3, true), (90, 5, true)];
        let up = find_destination(64, 5, Dir::Up, RANGE, true, (-64, 319), column(&col));
        assert_eq!(up, Some(90));
    }

    #[test]
    fn stops_at_range_and_world_bounds() {
        let col = [(75, 0, true), (-70, 0, true)];
        assert_eq!(
            find_destination(64, 0, Dir::Up, 10, false, (-64, 319), column(&col)),
            None
        );
        assert_eq!(
            find_destination(64, 0, Dir::Down, RANGE, false, (-64, 319), column(&col)),
            None
        );
    }
}
