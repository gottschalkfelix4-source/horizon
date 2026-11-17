//! Modern Dynamics for Horizon: item, fluid and energy pipes.
//!
//! Written from Modern Dynamics' observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). Released under LGPL-3.0-only, as the original. What the port
//! does, and what a harness against the NeoForge original still has to confirm, is in README.md.
//!
//! Pipes of one kind that touch form a network. Every pipe has a block entity of type
//! `moderndynamics:pipe`; the saved compound holds `servos`, a mask of the sides that carry a
//! servo. A servo on an item or fluid pipe pulls from the inventory or tank on its side every
//! [`SERVO_PERIOD`] ticks and sends what it pulled to the nearest inventory on the network that
//! takes it (fewest pipes away, then the order the network was walked in). An energy cable
//! network moves up to its tier's rate each tick from every energy handler beside it into every
//! other one.
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Mutex, OnceLock};

use mod_api::{
    move_energy, move_resources, BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent,
    BlockEntityTickReply, BlockEntityTicksEvent, BlockKey, BlockPart, CapabilityKey, Compound,
    Direction, HostSide, HostTransaction, Hooks, Host, InteractionResult, ItemPart,
    LevelRef, LevelSide, LivingRef, Registrar, RegistrationContext, ResourceHandler, ServerHost,
    Tag, UseOnEvent, ENERGY, FLUIDS, ITEMS,
};

/// Ticks between two servo operations.
pub const SERVO_PERIOD: i64 = 20;
/// Items one servo operation moves.
pub const SERVO_ITEMS: u32 = 8;
/// Millibuckets one servo operation moves.
pub const SERVO_FLUID: u32 = 1000;
/// The most pipes one network walk visits.
pub const NETWORK_LIMIT: usize = 1024;

/// What a pipe carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Item,
    Fluid,
    /// An energy cable, with its tier's rate in FE per tick.
    Energy(u32),
}

/// The pipe blocks, by their path in the `moderndynamics` namespace.
pub const PIPES: [(&str, Kind); 7] = [
    ("item_pipe", Kind::Item),
    ("fluid_pipe", Kind::Fluid),
    ("lv_cable", Kind::Energy(128)),
    ("mv_cable", Kind::Energy(512)),
    ("hv_cable", Kind::Energy(2048)),
    ("ev_cable", Kind::Energy(8192)),
    ("superconductor", Kind::Energy(1_000_000)),
];

/// Direction order as vanilla numbers them (down, up, north, south, west, east).
pub const DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

fn side_index(dir: Direction) -> usize {
    DIRECTIONS.iter().position(|d| *d == dir).unwrap_or(0)
}

fn normal(dir: Direction) -> [i32; 3] {
    match dir {
        Direction::Down => [0, -1, 0],
        Direction::Up => [0, 1, 0],
        Direction::North => [0, 0, -1],
        Direction::South => [0, 0, 1],
        Direction::West => [-1, 0, 0],
        Direction::East => [1, 0, 0],
    }
}

fn offset(pos: [i32; 3], dir: Direction) -> [i32; 3] {
    let n = normal(dir);
    [pos[0] + n[0], pos[1] + n[1], pos[2] + n[2]]
}

/// The sides of a pipe that carry a servo, one bit per side in [`DIRECTIONS`] order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Servos(pub u8);

impl Servos {
    pub fn has(self, dir: Direction) -> bool {
        self.0 & (1 << side_index(dir)) != 0
    }
    pub fn with(self, dir: Direction) -> Self {
        Self(self.0 | (1 << side_index(dir)))
    }
    pub fn sides(self) -> impl Iterator<Item = Direction> {
        DIRECTIONS.into_iter().filter(move |d| self.has(*d))
    }
    pub fn load(nbt: &Compound) -> Self {
        match nbt.get(&"servos".into()) {
            Some(Tag::Byte(b)) => Self(*b as u8 & 0x3f),
            Some(Tag::Int(i)) => Self(*i as u8 & 0x3f),
            _ => Self(0),
        }
    }
    pub fn save(self, nbt: &mut Compound) {
        nbt.insert("servos".into(), Tag::Byte(self.0 as i8));
    }
}

/// The pipes connected to `start` (itself first), walked breadth first through faces, with how
/// many pipes away each is. `is_pipe` says whether a position holds a pipe of the network's kind.
pub fn network(start: [i32; 3], mut is_pipe: impl FnMut([i32; 3]) -> bool, limit: usize) -> Vec<([i32; 3], u32)> {
    let mut seen = HashSet::from([start]);
    let mut queue = VecDeque::from([(start, 0u32)]);
    let mut out = Vec::new();
    while let Some((pos, dist)) = queue.pop_front() {
        out.push((pos, dist));
        if out.len() >= limit {
            break;
        }
        for dir in DIRECTIONS {
            let next = offset(pos, dir);
            if !seen.contains(&next) && is_pipe(next) {
                seen.insert(next);
                queue.push_back((next, dist + 1));
            }
        }
    }
    out
}

/// The places a network can deliver to, nearest first: each (pipe, side) whose neighbour is not
/// a pipe, leaving out the `source` side and sides with a servo (a servo side only extracts).
pub fn destinations(
    net: &[([i32; 3], u32)],
    pipes: &HashSet<[i32; 3]>,
    servos: &HashMap<[i32; 3], Servos>,
    source: ([i32; 3], Direction),
) -> Vec<([i32; 3], Direction)> {
    let mut out: Vec<(u32, usize, [i32; 3], Direction)> = Vec::new();
    for (order, (pos, dist)) in net.iter().enumerate() {
        for dir in DIRECTIONS {
            if pipes.contains(&offset(*pos, dir)) || (*pos, dir) == source {
                continue;
            }
            if servos.get(pos).is_some_and(|s| s.has(dir)) {
                continue;
            }
            out.push((*dist, order, *pos, dir));
        }
    }
    out.sort_by_key(|(d, o, _, _)| (*d, *o));
    out.into_iter().map(|(_, _, p, d)| (p, d)).collect()
}

struct Keys {
    blocks: Vec<(BlockKey, Kind)>,
}

static KEYS: OnceLock<Keys> = OnceLock::new();

fn kind_of(block: BlockKey) -> Option<Kind> {
    KEYS.get()?
        .blocks
        .iter()
        .find_map(|(k, kind)| (*k == block).then_some(*kind))
}

/// Energy networks already run this game tick (by dimension and game time), so each runs once.
static ENERGY_DONE: Mutex<Option<(String, i64, HashSet<[i32; 3]>)>> = Mutex::new(None);

fn servos_at(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Servos {
    host.block_entity(level, pos)
        .and_then(|be| be.data(host))
        .map_or(Servos(0), |nbt| Servos::load(&nbt))
}

/// One servo operation: pull from the holder on `side` of the pipe at `pipe` and deliver to the
/// nearest holder on the network that takes it.
fn servo<R: 'static>(
    host: &mut dyn ServerHost,
    level: LevelRef,
    key: &CapabilityKey<dyn ResourceHandler<R>>,
    pipe: [i32; 3],
    side: Direction,
    own: Servos,
    block: BlockKey,
    max: u32,
) {
    let Some(from) = level.capability(host, key, offset(pipe, side), Some(side.opposite())) else {
        return;
    };
    let net = network(pipe, |p| level.block(&*host, p) == Some(block), NETWORK_LIMIT);
    let pipes: HashSet<[i32; 3]> = net.iter().map(|(p, _)| *p).collect();
    let servos: HashMap<[i32; 3], Servos> = net
        .iter()
        .map(|(p, _)| (*p, if *p == pipe { own } else { servos_at(&*host, level, *p) }))
        .collect();
    for (pos, dir) in destinations(&net, &pipes, &servos, (pipe, side)) {
        let Some(to) = level.capability(host, key, offset(pos, dir), Some(dir.opposite())) else {
            continue;
        };
        let moved = host
            .transaction(|host, tx| move_resources(host, &from, &to, |_| true, max, tx))
            .unwrap_or(0);
        if moved > 0 {
            return;
        }
    }
}

/// One tick of the energy network that holds the cable at `pipe`, if no cable of it ran yet.
fn energy_tick(host: &mut dyn ServerHost, level: LevelRef, pipe: [i32; 3], block: BlockKey, rate: u32) {
    let Some(time) = level.game_time(&*host) else {
        return;
    };
    let dim = host.dimension().to_owned();
    {
        let Ok(mut done) = ENERGY_DONE.lock() else {
            return;
        };
        match done.as_mut() {
            Some((d, t, set)) if *d == dim && *t == time => {
                if set.contains(&pipe) {
                    return;
                }
            }
            _ => *done = Some((dim.clone(), time, HashSet::new())),
        }
    }
    let net = network(pipe, |p| level.block(&*host, p) == Some(block), NETWORK_LIMIT);
    let pipes: HashSet<[i32; 3]> = net.iter().map(|(p, _)| *p).collect();
    if let Ok(mut done) = ENERGY_DONE.lock() {
        if let Some((_, _, set)) = done.as_mut() {
            set.extend(pipes.iter().copied());
        }
    }
    let mut ends = Vec::new();
    for (pos, _) in &net {
        for dir in DIRECTIONS {
            let at = offset(*pos, dir);
            if pipes.contains(&at) {
                continue;
            }
            if let Some(cap) = level.capability(host, &ENERGY, at, Some(dir.opposite())) {
                ends.push((at, cap));
            }
        }
    }
    let mut budget = rate;
    for (i, (from_pos, from)) in ends.iter().enumerate() {
        for (j, (to_pos, to)) in ends.iter().enumerate() {
            if budget == 0 {
                return;
            }
            if i == j || from_pos == to_pos {
                continue;
            }
            let moved = host
                .transaction(|host, tx| move_energy(host, from, to, budget, tx))
                .unwrap_or(0);
            budget -= moved.min(budget);
        }
    }
}

struct Pipe;

fn is_ours(block: &str) -> bool {
    block
        .strip_prefix("moderndynamics:")
        .is_some_and(|name| PIPES.iter().any(|(n, _)| *n == name))
}

#[mod_api::horizon_part]
impl BlockPart for Pipe {}

#[mod_api::horizon_part]
impl BlockEntityPart for Pipe {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        is_ours(event.block).then(|| {
            let mut nbt = Compound::new();
            Servos(0).save(&mut nbt);
            nbt
        })
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if is_ours(event.block) {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        // Nothing of the pipe's own changes here: its neighbours' storages do.
        *event.reply = BlockEntityTickReply::Unchanged;
        let pos = event.pos;
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        let Some(block) = level.block(&*host, pos) else {
            return;
        };
        let Some(kind) = kind_of(block) else {
            return;
        };
        match kind {
            Kind::Energy(rate) => energy_tick(host, level, pos, block, rate),
            Kind::Item | Kind::Fluid => {
                let own = Servos::load(&event.entity.nbt);
                if own.0 == 0 {
                    return;
                }
                let time = level.game_time(&*host).unwrap_or(0);
                if time.rem_euclid(SERVO_PERIOD) != 0 {
                    return;
                }
                for side in own.sides() {
                    if kind == Kind::Item {
                        servo(host, level, &ITEMS, pos, side, own, block, SERVO_ITEMS);
                    } else {
                        servo(host, level, &FLUIDS, pos, side, own, block, SERVO_FLUID);
                    }
                }
            }
        }
    }
}

/// The servo item: using it on an item or fluid pipe fits it on the first side (down, up, north,
/// south, west, east) that touches something with storage of the pipe's kind and has no servo yet.
struct ServoItem;

#[mod_api::horizon_part]
impl ItemPart for ServoItem {
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        let level = host.level(host.dimension())?;
        let pos = event.pos;
        let kind = level.block(&*host, pos).and_then(kind_of)?;
        if !matches!(kind, Kind::Item | Kind::Fluid) {
            return None;
        }
        let entity = host.block_entity(level, pos)?;
        let servos = entity.data(&*host).map_or(Servos(0), |nbt| Servos::load(&nbt));
        let side = DIRECTIONS.into_iter().find(|dir| {
            if servos.has(*dir) || level.block(&*host, offset(pos, *dir)).and_then(kind_of).is_some() {
                return false;
            }
            let at = offset(pos, *dir);
            match kind {
                Kind::Item => level.capability(host, &ITEMS, at, Some(dir.opposite())).is_some(),
                _ => level.capability(host, &FLUIDS, at, Some(dir.opposite())).is_some(),
            }
        })?;
        let mut patch = Compound::new();
        servos.with(side).save(&mut patch);
        if !matches!(entity.merge_data(host, patch), Ok(true)) {
            return Some(InteractionResult::Fail);
        }
        let player = host.player(event.player)?;
        if player.has_infinite_materials(&*host) != Some(true) {
            let living = LivingRef::new(player.entity());
            let _ = living.set_hand_item(host, event.hand, event.stack.clone().shrink(1));
        }
        Some(InteractionResult::SuccessServer)
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let ids: Vec<String> = PIPES.iter().map(|(n, _)| format!("moderndynamics:{n}")).collect();
        let names: Vec<&str> = ids.iter().map(String::as_str).collect();
        let blocks = names
            .iter()
            .zip(PIPES)
            .map(|(id, (_, kind))| (r.block_key(id), kind))
            .collect();
        let servo = r.item_key("moderndynamics:servo");
        r.item_part(servo, ServoItem);
        let _ = KEYS.set(Keys { blocks });
        let kind = r.block_entity_type("pipe").blocks(&names).build();
        let _ = r.block_entity_part(&kind, Pipe);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(n: i32) -> HashSet<[i32; 3]> {
        (0..n).map(|x| [x, 64, 0]).collect()
    }

    #[test]
    fn a_network_is_walked_breadth_first_through_touching_pipes_only() {
        let mut pipes = line(4);
        pipes.insert([1, 65, 0]);
        pipes.insert([10, 64, 0]); // not touching
        let net = network([0, 64, 0], |p| pipes.contains(&p), 100);
        let dist: HashMap<[i32; 3], u32> = net.iter().copied().collect();
        assert_eq!(net.len(), 5);
        assert_eq!(dist[&[3, 64, 0]], 3);
        assert_eq!(dist[&[1, 65, 0]], 2);
        assert!(!dist.contains_key(&[10, 64, 0]));
        assert_eq!(network([0, 64, 0], |p| pipes.contains(&p), 2).len(), 2);
    }

    #[test]
    fn delivery_goes_to_the_nearest_side_and_never_back_into_the_source_or_a_servo() {
        let pipes = line(3);
        let net = network([0, 64, 0], |p| pipes.contains(&p), 100);
        let mut servos = HashMap::new();
        servos.insert([0, 64, 0], Servos(0).with(Direction::West));
        servos.insert([1, 64, 0], Servos(0).with(Direction::Up));
        let dests = destinations(&net, &pipes, &servos, ([0, 64, 0], Direction::West));
        // The source pipe's other free sides come first (distance 0), then pipe 1's, then pipe 2's.
        assert_eq!(dests[0], ([0, 64, 0], Direction::Down));
        assert!(!dests.contains(&([0, 64, 0], Direction::West)));
        assert!(!dests.contains(&([1, 64, 0], Direction::Up)));
        assert!(!dests.contains(&([0, 64, 0], Direction::East)), "the next pipe is not a destination");
        assert_eq!(dests.last(), Some(&([2, 64, 0], Direction::East)));
        assert_eq!(dests.len(), 4 + 3 + 5);
    }

    #[test]
    fn servo_sides_round_trip_through_their_saved_form() {
        let s = Servos(0).with(Direction::North).with(Direction::East);
        let mut nbt = Compound::new();
        s.save(&mut nbt);
        let back = Servos::load(&nbt);
        assert_eq!(back, s);
        assert_eq!(back.sides().collect::<Vec<_>>(), [Direction::North, Direction::East]);
    }
}
