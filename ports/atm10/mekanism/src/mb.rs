//! What the multiblocks with a master valve share: finding the master from any valve, and the
//! once-a-second structure check a valve's tick runs. The master is the lowest valve of the
//! structure; it keeps the contents.
use mod_api::{
    BlockEntityRef, BlockEntityTickEvent, BlockEntityTickReply, BlockSide, Compound, Host, LevelRef,
    ServerHost, Tag,
};

use crate::multiblock::{self, Found, Pos};

pub fn long(nbt: &Compound, key: &str) -> i64 {
    match nbt.get(&key.into()) {
        Some(Tag::Long(v)) => *v,
        Some(Tag::Int(v)) => i64::from(*v),
        _ => 0,
    }
}

pub fn set_long(nbt: &mut Compound, key: &str, v: i64) {
    nbt.insert(key.into(), Tag::Long(v));
}

pub fn id_at(host: &dyn Host, level: LevelRef, p: Pos) -> Option<String> {
    level.block(host, p).map(|k| k.id(host))
}

pub fn is_air(id: Option<&str>) -> bool {
    matches!(id, None | Some("minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"))
}

/// The master's handle and data for the valve at `at`, while its structure is formed.
pub fn master(host: &mut dyn ServerHost, at: BlockSide) -> Option<(BlockEntityRef, Compound)> {
    let own = host.block_entity(at.level, at.pos)?.data(&*host)?;
    if long(&own, "formed") != 1 {
        return None;
    }
    let pos = [long(&own, "mx") as i32, long(&own, "my") as i32, long(&own, "mz") as i32];
    let entity = host.block_entity(at.level, pos)?;
    let data = entity.data(&*host)?;
    Some((entity, data))
}

/// The tick of a valve: once a second it looks for its structure (a box whose shell `shell`
/// accepts and whose inside `inside` accepts), records the master's position, and on the master
/// calls `formed` to count what is inside. Every tick the master runs `work` on its own data.
pub fn tick_valve(
    host: &mut dyn ServerHost,
    event: &mut BlockEntityTickEvent<'_>,
    valve: &str,
    shell: impl Fn(Option<&str>) -> bool,
    inside: impl Fn(Option<&str>) -> bool,
    formed: impl FnOnce(&dyn Host, LevelRef, &Found, &mut Compound),
    work: impl FnOnce(&mut Compound),
) {
    let pos = event.pos;
    let mut nbt = event.entity.nbt.clone();
    let age = long(&nbt, "age") + 1;
    set_long(&mut nbt, "age", age % 20);
    let Some(level) = host.level(host.dimension()) else { return };
    let mut is_master = long(&nbt, "formed") == 1
        && [long(&nbt, "mx") as i32, long(&nbt, "my") as i32, long(&nbt, "mz") as i32] == pos;
    if age % 20 == 1 {
        let found = multiblock::find(
            pos,
            |p| shell(id_at(&*host, level, p).as_deref()),
            |p| inside(id_at(&*host, level, p).as_deref()),
        );
        match found {
            Some(found) => {
                let master = found
                    .shell
                    .iter()
                    .copied()
                    .find(|p| id_at(&*host, level, *p).as_deref() == Some(valve))
                    .unwrap_or(pos);
                set_long(&mut nbt, "formed", 1);
                set_long(&mut nbt, "mx", i64::from(master[0]));
                set_long(&mut nbt, "my", i64::from(master[1]));
                set_long(&mut nbt, "mz", i64::from(master[2]));
                is_master = master == pos;
                if is_master {
                    formed(&*host, level, &found, &mut nbt);
                }
            }
            None => {
                set_long(&mut nbt, "formed", 0);
                set_long(&mut nbt, "capacity", 0);
                is_master = false;
            }
        }
    }
    if is_master {
        work(&mut nbt);
    }
    if nbt == event.entity.nbt {
        *event.reply = BlockEntityTickReply::Unchanged;
        return;
    }
    event.entity.nbt = nbt;
}
