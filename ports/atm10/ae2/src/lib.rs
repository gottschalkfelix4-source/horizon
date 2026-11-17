//! Applied Energistics 2 for Horizon, stage 1: the ME network's blocks (cables in 17 colours,
//! controller), its power (energy acceptor, energy cells) and the network model itself.
//!
//! Written from AE2's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). README.md lists what is done and what a check against the
//! NeoForge original still has to confirm.
//!
//! A network is every AE block joined by faces (see `rules::connects`). Its lowest block runs it each
//! tick: energy acceptors hand FE to the cells, the idle draw of cables and the controller is taken
//! from the cells, and the result (`powered`, `nodes`, `stored`) is saved in that block's data.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Compound, Direction, ENERGY, Hooks, Host, LevelRef,
    LevelSide, HostTransaction, ModEnergyBuffer, Registrar, RegistrationContext, ServerHost, Tag,
};

pub mod rules;
use rules::*;

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

fn set_int(nbt: &mut Compound, key: &str, v: i32) {
    nbt.insert(key.into(), Tag::Int(v));
}

fn offset(pos: [i32; 3], dir: Direction) -> [i32; 3] {
    let n = dir.normal();
    [pos[0] + n[0], pos[1] + n[1], pos[2] + n[2]]
}

fn id_at(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<String> {
    Some(level.block(host, pos)?.id(host))
}

/// Moves up to `budget` FE from `from`'s energy to `to`'s in one transaction; how much moved.
fn route(
    host: &mut dyn ServerHost,
    level: LevelRef,
    from: [i32; 3],
    to: [i32; 3],
    budget: u32,
) -> u32 {
    let (Some(src), Some(dst)) = (
        level.capability(host, &ENERGY, from, Some(Direction::Up)),
        level.capability(host, &ENERGY, to, Some(Direction::Down)),
    ) else {
        return 0;
    };
    // what the source can give, found in a transaction that is thrown away
    let held = host
        .transaction(|host, tx| {
            let have = src.amount(host).min(budget);
            let n = src.extract(host, tx, have);
            Err::<(), u32>(n)
        })
        .err()
        .unwrap_or(0);
    if held == 0 {
        return 0;
    }
    host.transaction(|host, tx| {
        let taken = dst.insert(host, tx, held);
        if taken == 0 {
            return Ok(0);
        }
        if src.extract(host, tx, taken) < taken {
            return Err(());
        }
        Ok::<u32, ()>(taken)
    })
    .unwrap_or(0)
}

/// Takes `fe` from the cells, all or nothing.
fn draw(host: &mut dyn ServerHost, level: LevelRef, cells: &[[i32; 3]], fe: u32) -> bool {
    if fe == 0 {
        return true;
    }
    host.transaction(|host, tx| {
        let mut left = fe;
        for &c in cells {
            if left == 0 {
                break;
            }
            let Some(cap) = level.capability(host, &ENERGY, c, Some(Direction::Up)) else {
                continue;
            };
            let have = cap.amount(host).min(left);
            left -= cap.extract(host, tx, have);
        }
        if left == 0 { Ok(()) } else { Err(()) }
    })
    .is_ok()
}

fn stored(host: &mut dyn ServerHost, level: LevelRef, cells: &[[i32; 3]]) -> i64 {
    let mut sum = 0i64;
    for &c in cells {
        if let Some(cap) = level.capability(host, &ENERGY, c, Some(Direction::Up)) {
            sum += cap.amount(host) as i64;
        }
    }
    sum
}

/// One tick of the network `pos` leads; returns what the leader saves.
fn tick_network(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    carried: i64,
) -> Option<(usize, bool, i64, i64)> {
    let nodes = led_network(pos, |p| {
        let Some(here) = id_at(&*host, level, p) else { return Vec::new() };
        Direction::ALL
            .into_iter()
            .map(|d| offset(p, d))
            .filter(|n| id_at(&*host, level, *n).is_some_and(|o| connects(&here, &o)))
            .collect()
    })?;
    let (mut cells, mut acceptors, mut idle) = (Vec::new(), Vec::new(), 0i64);
    for &n in &nodes {
        match id_at(&*host, level, n).as_deref().and_then(parse_block) {
            Some(Kind::Device(Device::EnergyAcceptor)) => acceptors.push(n),
            Some(Kind::Device(d)) if d.is_cell() => cells.push(n),
            Some(k) => idle += k.idle_centi_fe(),
            None => {}
        }
    }
    // what the acceptors took in goes into the cells
    for &a in &acceptors {
        for &c in &cells {
            route(host, level, a, c, u32::MAX);
        }
    }
    let (take, rem) = drain(idle, carried);
    let ok = powered(stored(host, level, &cells), take) && draw(host, level, &cells, take as u32);
    // an unpowered network keeps the carried hundredths; nothing was taken
    let rem = if ok { rem } else { carried };
    Some((nodes.len(), ok, stored(host, level, &cells), rem))
}

/// One part for every AE2 block entity; the block's id says which kind it is.
struct Network;

#[mod_api::horizon_part]
impl BlockPart for Network {}

#[mod_api::horizon_part]
impl BlockEntityPart for Network {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        parse_block(event.block).map(|_| {
            let mut nbt = Compound::new();
            set_int(&mut nbt, "energy", 0);
            nbt
        })
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if parse_block(event.block).is_some() {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let Some(level) = host.level(host.dimension()) else { return };
        let carried = int(&event.entity.nbt, "drain_rem") as i64;
        let Some((nodes, powered, stored, rem)) = tick_network(host, level, pos, carried) else {
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        };
        let mut nbt = event.entity.nbt.clone();
        set_int(&mut nbt, "nodes", nodes as i32);
        set_int(&mut nbt, "powered", i32::from(powered));
        set_int(&mut nbt, "stored", stored.min(i32::MAX as i64) as i32);
        set_int(&mut nbt, "drain_rem", rem as i32);
        if nbt == event.entity.nbt {
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        }
        event.entity.nbt = nbt;
    }
}

/// The game's energy buffer for a block: cells and the acceptor move FE through every face
/// (the acceptor only takes it in from outside; the network hands it on to the cells).
fn buffer_of(d: Device) -> Option<ModEnergyBuffer> {
    let cap = d.capacity_fe();
    (cap > 0).then(|| ModEnergyBuffer {
        capacity: cap,
        max_insert: cap,
        max_extract: cap,
        insert_faces: [true; 6],
        extract_faces: [true; 6],
    })
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let mut groups: Vec<(String, Vec<String>)> = Vec::new();
        for c in Cable::ALL {
            groups.push((c.name().into(), COLORS.iter().map(|col| cable_id(col, c)).collect()));
        }
        for d in Device::ALL {
            groups.push((d.name().into(), vec![device_id(d)]));
        }
        for (entity_type, ids) in groups {
            for id in &ids {
                if let Some(Kind::Device(d)) = parse_block(id) {
                    if let Some(buf) = buffer_of(d) {
                        let key = r.block_key(id);
                        r.block_energy(&key, buf);
                    }
                }
            }
            let names: Vec<&str> = ids.iter().map(String::as_str).collect();
            let built = r.block_entity_type(&entity_type).blocks(&names).build();
            let _ = r.block_entity_part(&built, Network);
        }
    }
}
