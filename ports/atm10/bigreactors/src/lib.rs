//! Extreme Reactors for Horizon: the passive fission reactor as a multiblock.
//!
//! Written from Extreme Reactors' observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). README.md lists what is ported and which numbers a check
//! against the NeoForge original still has to confirm.
//!
//! The controller (`bigreactors:reactor_controller`) is the only block with a block entity. Every
//! second it checks the cuboid of reactor parts around it (`rules::validate`); while assembled it
//! burns the yellorium fed into its two slots (fuel in, cyanite waste out), turns the heat of the
//! fuel rods into energy and pushes it out through the power taps.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockKey, BlockPart, Compound, Direction, Hooks, Host, HostTransaction,
    Identifier, InteractionResult, ItemStack, LevelRef, LevelSide, ModContainer, ModEnergyBuffer,
    Registrar, RegistrationContext, ServerHost, Tag, UseWithoutItemEvent, ENERGY,
};
use std::sync::OnceLock;

pub mod rules;
use rules::*;

/// Slots: yellorium in, cyanite out.
const FUEL_SLOT: usize = 0;
const WASTE_SLOT: usize = 1;
const BUFFER: i32 = 1_000_000;
const CHECK_PERIOD: i64 = 20;
const IDS: [&str; 5] = [
    "bigreactors:reactor_casing",
    "bigreactors:reactor_controller",
    "bigreactors:reactor_power_tap",
    "bigreactors:reactor_fuel_rod",
    "bigreactors:reactor_control_rod",
];
const PARTS: [Part; 5] = [Part::Casing, Part::Controller, Part::PowerTap, Part::FuelRod, Part::ControlRod];

static KEYS: OnceLock<Vec<BlockKey>> = OnceLock::new();

fn part_at(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Part {
    let (Some(keys), Some(block)) = (KEYS.get(), level.block(host, pos)) else {
        return Part::Other;
    };
    keys.iter().position(|k| *k == block).map_or(Part::Other, |i| PARTS[i])
}

fn num(nbt: &Compound, key: &str) -> f64 {
    match nbt.get(&key.into()) {
        Some(Tag::Double(v)) => f64::from_bits(*v),
        _ => 0.0,
    }
}

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

fn offset(pos: [i32; 3], dir: Direction) -> [i32; 3] {
    let n = dir.normal();
    [pos[0] + n[0], pos[1] + n[1], pos[2] + n[2]]
}

struct Controller;

#[mod_api::horizon_part]
impl BlockPart for Controller {
    /// Using the controller steps the control rods 10 % further in (back to 0 after 100).
    fn use_without_item(&self, host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
        let level = host.level(host.dimension())?;
        let entity = host.block_entity(level, event.click.pos)?;
        let data = entity.data(&*host)?;
        let next = (int(&data, "insertion") + 10) % 110;
        let mut patch = Compound::new();
        patch.insert("insertion".into(), Tag::Int(next.min(100)));
        let _ = entity.merge_data(host, patch);
        let player = host.player(event.click.player)?;
        let line = mod_api::Text::translate(
            "message.bigreactors.control_rods",
            vec![mod_api::Text::literal(next.min(100).to_string())],
        );
        let _ = player.action_bar(host, line);
        Some(InteractionResult::SuccessServer)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for Controller {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == IDS[1]).then(|| {
            let mut nbt = Compound::new();
            nbt.insert("energy".into(), Tag::Int(0));
            nbt
        })
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == IDS[1] {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let Some(level) = host.level(host.dimension()) else { return };
        if part_at(&*host, level, pos) != Part::Controller {
            return;
        }
        let time = level.game_time(&*host).unwrap_or(0);
        let mut nbt_local = event.entity.nbt.clone();
        let mut items_local = event.entity.items.clone();
        let nbt = &mut nbt_local;
        if items_local.len() < 2 {
            items_local.resize(2, None);
        }
        // the structure, checked each second and kept as rods, columns and tap positions
        if (time + i64::from(pos[0] ^ pos[2])).rem_euclid(CHECK_PERIOD) == 0 || !nbt.contains_key(&"checked".into()) {
            let s = validate(pos, |p| part_at(&*host, level, p));
            nbt.insert("checked".into(), Tag::Byte(1));
            match &s {
                Ok(s) => {
                    nbt.insert("rods".into(), Tag::Int(s.rods as i32));
                    nbt.insert("columns".into(), Tag::Int(s.columns as i32));
                    let flat: Vec<i32> = s.taps.iter().flatten().copied().collect();
                    nbt.insert("taps".into(), Tag::IntArray(flat.into()));
                }
                Err(_) => {
                    nbt.insert("rods".into(), Tag::Int(0));
                    nbt.insert("columns".into(), Tag::Int(0));
                    nbt.insert("taps".into(), Tag::IntArray(Vec::new().into()));
                }
            }
        }
        let (rods, columns) = (int(nbt, "rods") as u32, int(nbt, "columns") as u32);
        let mut core = Core { fuel: num(nbt, "fuel"), waste: num(nbt, "waste"), heat: if nbt.contains_key(&"heat".into()) { num(nbt, "heat") } else { AMBIENT } };
        let mut energy = int(nbt, "energy");
        if rods > 0 {
            // feed: one yellorium ingot whenever the tank has room, and hand out cyanite when 1000 units of waste collected
            let items = &mut items_local;
            if core.fuel_room(columns) >= f64::from(UNITS_PER_INGOT) {
                let is_fuel = items[FUEL_SLOT].as_ref().is_some_and(|s| s.item().id(&*host) == "bigreactors:yellorium_ingot");
                if is_fuel {
                    items[FUEL_SLOT] = items[FUEL_SLOT].take().and_then(|s| s.shrink(1));
                    core.fuel += f64::from(UNITS_PER_INGOT);
                }
            }
            if core.waste >= f64::from(UNITS_PER_INGOT) {
                let key = Identifier::parse("bigreactors:cyanite_ingot").ok().and_then(|i| host.item_key(&i));
                if let Some(cyanite) = key.and_then(|k| ItemStack::new(k, 1)) {
                    let max = host.max_stack_size(cyanite.item()).unwrap_or(64);
                    match &mut items[WASTE_SLOT] {
                        None => {
                            items[WASTE_SLOT] = Some(cyanite);
                            core.waste -= f64::from(UNITS_PER_INGOT);
                        }
                        Some(held) if held.is_stackable_with(&cyanite) && held.count() < max => {
                            let _ = held.grow(1);
                            core.waste -= f64::from(UNITS_PER_INGOT);
                        }
                        _ => {}
                    }
                }
            }
            let running = !level.has_neighbor_signal(&*host, pos);
            let made = core.tick(rods, int(nbt, "insertion"), running);
            energy = (energy + made as i32).min(BUFFER);
        } else {
            core.heat = AMBIENT;
        }
        // push through the taps
        let taps: Vec<[i32; 3]> = match nbt.get(&"taps".into()) {
            Some(Tag::IntArray(a)) => a.chunks(3).filter(|c| c.len() == 3).map(|c| [c[0], c[1], c[2]]).collect(),
            _ => Vec::new(),
        };
        for tap in taps {
            if energy <= 0 {
                break;
            }
            for dir in Direction::ALL {
                let n = offset(tap, dir);
                if part_at(&*host, level, n) != Part::Other {
                    continue;
                }
                let Some(cap) = level.capability(host, &ENERGY, n, Some(dir.opposite())) else { continue };
                let want = energy as u32;
                let moved = host.transaction(|host, tx| Ok::<u32, ()>(cap.insert(host, tx, want))).unwrap_or(0);
                energy -= moved as i32;
            }
        }
        nbt.insert("fuel".into(), Tag::Double(core.fuel.to_bits()));
        nbt.insert("waste".into(), Tag::Double(core.waste.to_bits()));
        nbt.insert("heat".into(), Tag::Double(core.heat.to_bits()));
        nbt.insert("energy".into(), Tag::Int(energy));
        let lit = i8::from(rods > 0 && core.fuel > 0.0);
        if int(nbt, "lit") != i32::from(lit) {
            nbt.insert("lit".into(), Tag::Int(i32::from(lit)));
            host.set_block_property(pos, "lit", if lit == 1 { "true" } else { "false" });
        }
        if nbt_local == event.entity.nbt && items_local == event.entity.items {
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        }
        event.entity.nbt = nbt_local;
        event.entity.items = items_local;
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let keys: Vec<BlockKey> = IDS.iter().map(|id| r.block_key(id)).collect();
        let controller = keys[1].clone();
        r.block_energy(
            &controller,
            ModEnergyBuffer { capacity: BUFFER, max_insert: 0, max_extract: 0, insert_faces: [false; 6], extract_faces: [false; 6] },
        );
        r.block_container(
            &controller,
            ModContainer {
                slots: 5,
                faces: std::array::from_fn(|_| vec![FUEL_SLOT, WASTE_SLOT]),
                inputs: vec![FUEL_SLOT],
                outputs: vec![WASTE_SLOT],
                menu: Some("minecraft:hopper".into()),
            },
        );
        let _ = KEYS.set(keys);
        let kind = r.block_entity_type("reactor_controller").blocks(&[IDS[1]]).build();
        let _ = r.block_entity_part(&kind, Controller);
    }
}
