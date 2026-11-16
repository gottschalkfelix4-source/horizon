//! More Industrial Foregoing Addons for Horizon: the Lava Generator.
//!
//! Written from the mod's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). Confidence in this port is low: see README.md.
//!
//! A generator burns lava from its tank at [`MB_PER_TICK`] for [`FE_PER_TICK`] while its energy
//! buffer has room, and pushes energy to every neighbour that takes it.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Compound, Direction, ENERGY, Hooks, Host, HostTransaction,
    LevelSide, ModEnergyBuffer, ModFluidTanks, Registrar, RegistrationContext, ServerHost, Tag,
};

pub const TANK: i32 = 8_000;
pub const CAPACITY: i32 = 500_000;
pub const SEND_PER_TICK: i32 = 2_000;
pub const MB_PER_TICK: i32 = 5;
pub const FE_PER_TICK: i32 = 120;

/// What one tick of the generator does, from its tank and buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub burned: i32,
    pub made: i32,
}

/// One tick: burns lava when the tank holds `lava` millibuckets and the buffer (`energy` of
/// `CAPACITY`) has room for a tick's output; a part-tick of lava is burned as the whole.
pub fn burn(lava: i32, energy: i32) -> Step {
    if lava < MB_PER_TICK || energy + FE_PER_TICK > CAPACITY {
        return Step { burned: 0, made: 0 };
    }
    Step { burned: MB_PER_TICK, made: FE_PER_TICK }
}

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

/// The lava in the saved tank: only `minecraft:lava` counts.
pub fn lava_in(nbt: &Compound) -> i32 {
    if let Some(Tag::List(list)) = nbt.get(&"fluids".into()) {
        if let Some(t) = list.first() {
            let is_lava = matches!(t.get("Fluid"), Some(Tag::String(f)) if f.to_string_lossy() == "minecraft:lava");
            if let (true, Some(Tag::Int(a))) = (is_lava, t.get("Amount")) {
                return (*a).max(0);
            }
        }
    }
    0
}

fn set_lava(nbt: &mut Compound, amount: i32) {
    let fluid = if amount > 0 { "minecraft:lava" } else { "minecraft:empty" };
    let t = Tag::compound([("Fluid", Tag::String(fluid.into())), ("Amount", Tag::Int(amount))]);
    nbt.insert("fluids".into(), Tag::List(vec![t]));
}

struct Generator;

#[mod_api::horizon_part]
impl BlockPart for Generator {}

#[mod_api::horizon_part]
impl BlockEntityPart for Generator {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == "mifa:lava_generator").then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == "mifa:lava_generator" {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let lava = lava_in(&event.entity.nbt);
        let mut energy = int(&event.entity.nbt, "energy");
        let step = burn(lava, energy);
        energy += step.made;
        let mut sent = 0;
        if energy > 0 {
            if let Some(level) = host.level(host.dimension()) {
                for dir in Direction::ALL {
                    let n = dir.normal();
                    let at = [event.pos[0] + n[0], event.pos[1] + n[1], event.pos[2] + n[2]];
                    let Some(cap) = level.capability(host, &ENERGY, at, Some(dir.opposite())) else { continue };
                    let offer = (energy - sent).min(SEND_PER_TICK).max(0) as u32;
                    if offer == 0 {
                        break;
                    }
                    sent += host
                        .transaction(|host, tx| Ok::<u32, ()>(cap.insert(host, tx, offer)))
                        .unwrap_or(0) as i32;
                }
            }
        }
        energy -= sent;
        if step.burned == 0 && sent == 0 {
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        }
        if step.burned > 0 {
            set_lava(&mut event.entity.nbt, lava - step.burned);
        }
        event.entity.nbt.insert("energy".into(), Tag::Int(energy));
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let key = r.block_key("mifa:lava_generator");
        r.block_part(key, Generator);
        r.block_energy(&key, ModEnergyBuffer {
            capacity: CAPACITY,
            max_insert: 0,
            max_extract: SEND_PER_TICK,
            insert_faces: [false; 6],
            extract_faces: [true; 6],
        });
        let mut tanks = ModFluidTanks::default();
        tanks.tanks = vec![TANK];
        tanks.faces = std::array::from_fn(|_| vec![0]);
        tanks.inputs = vec![0];
        tanks.outputs = Vec::new();
        r.block_fluids(&key, tanks);
        let kind = r.block_entity_type("lava_generator").blocks(&["mifa:lava_generator"]).build();
        let _ = r.block_entity_part(&kind, Generator);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bucket_of_lava_lasts_200_ticks_and_makes_24000_fe() {
        let (mut lava, mut energy, mut ticks) = (1000, 0, 0);
        loop {
            let s = burn(lava, energy);
            if s.burned == 0 {
                break;
            }
            lava -= s.burned;
            energy += s.made;
            ticks += 1;
        }
        assert_eq!((ticks, lava, energy), (200, 0, 24_000));
    }

    #[test]
    fn a_full_buffer_stops_the_burning_and_only_lava_counts() {
        assert_eq!(burn(1000, CAPACITY - FE_PER_TICK + 1), Step { burned: 0, made: 0 });
        let mut nbt = Compound::new();
        set_lava(&mut nbt, 700);
        assert_eq!(lava_in(&nbt), 700);
        nbt.insert(
            "fluids".into(),
            Tag::List(vec![Tag::compound([("Fluid", Tag::String("minecraft:water".into())), ("Amount", Tag::Int(700))])]),
        );
        assert_eq!(lava_in(&nbt), 0);
    }
}
