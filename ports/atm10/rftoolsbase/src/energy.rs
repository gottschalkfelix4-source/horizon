//! Forge Energy (RF) on Horizon: a block entity's buffer is the `energy` int of its saved
//! compound (`Registrar::block_energy`), and neighbours are reached through the `ENERGY`
//! capability, each move in a transaction of its own.
use mod_api::{Compound, Direction, ENERGY, HostTransaction, LevelRef, ServerHost, Tag};

/// The energy a block entity's compound holds.
pub fn stored(nbt: &Compound) -> i32 {
    match nbt.get(&"energy".into()) {
        Some(Tag::Int(v)) => *v,
        Some(Tag::Long(v)) => (*v).clamp(0, i64::from(i32::MAX)) as i32,
        _ => 0,
    }
}

/// Writes the energy back into the compound.
pub fn set_stored(nbt: &mut Compound, amount: i32) {
    nbt.insert("energy".into(), Tag::Int(amount.max(0)));
}

/// The block beside `pos` on `dir`.
pub fn beside(pos: [i32; 3], dir: Direction) -> [i32; 3] {
    let n = dir.normal();
    [pos[0] + n[0], pos[1] + n[1], pos[2] + n[2]]
}

/// What the block beside `pos` on `dir` holds and can hold, through the face that looks at us.
#[allow(dead_code)]
pub fn read(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], dir: Direction) -> Option<(u32, u32)> {
    let cap = level.capability(host, &ENERGY, beside(pos, dir), Some(dir.opposite()))?;
    Some((cap.amount(host), cap.capacity(host)))
}

/// Offers up to `max` to the block beside `pos` on `dir`: how much it took.
#[allow(dead_code)]
pub fn offer(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], dir: Direction, max: u32) -> u32 {
    if max == 0 {
        return 0;
    }
    let Some(cap) = level.capability(host, &ENERGY, beside(pos, dir), Some(dir.opposite())) else {
        return 0;
    };
    host.transaction(|host, tx| Ok::<u32, ()>(cap.insert(host, tx, max)))
        .unwrap_or(0)
}

/// Draws up to `max` from the block beside `pos` on `dir`: how much came.
#[allow(dead_code)]
pub fn draw(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], dir: Direction, max: u32) -> u32 {
    if max == 0 {
        return 0;
    }
    let Some(cap) = level.capability(host, &ENERGY, beside(pos, dir), Some(dir.opposite())) else {
        return 0;
    };
    host.transaction(|host, tx| Ok::<u32, ()>(cap.extract(host, tx, max)))
        .unwrap_or(0)
}

/// Sends up to `budget` out of every face in `faces` in turn: how much left in all.
#[allow(dead_code)]
pub fn push_all(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], faces: &[Direction], per_face: u32, budget: u32) -> u32 {
    let mut sent = 0;
    for &dir in faces {
        let left = budget - sent;
        if left == 0 {
            break;
        }
        sent += offer(host, level, pos, dir, left.min(per_face));
    }
    sent
}

/// Every face, in `Direction` order.
pub const ALL_FACES: [Direction; 6] = Direction::ALL;
