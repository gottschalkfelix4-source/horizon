//! RFToolsUtility for Horizon: the redstone Timer and Counter, the wireless Redstone Transmitter
//! and Receiver, and the Environmental Controller.
//!
//! Written from RFToolsUtility's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What is ported, and what a check against the NeoForge original
//! still has to confirm, is in README.md.
//!
//! The original sets each block in a screen. Here an empty-hand click steps the block's one
//! setting (delay, target, channel, radius) through a list and shows it on the action bar.
pub mod env;
pub mod logic;

use std::sync::Mutex;

use mod_api::{
    ActiveEffect, BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, BlockSignalEvent, Compound, Hooks, Host, InteractionResult,
    IsSignalSourceEvent, LevelHost, LevelRef, LevelSide, ModContainer, ModEnergyBuffer, Registrar,
    RegistrationContext, ServerHost, Tag, Text, UseWithoutItemEvent,
};

use crate::logic::{Channels, Counter, Timer};

const NS: &str = "rftoolsutility";
const TICKING: [&str; 5] = ["timer", "counter", "redstone_transmitter", "redstone_receiver", "environmental_controller"];
const MODULE_SLOTS: usize = 9;

static CHANNELS: Mutex<Channels> = Mutex::new(Channels::new());

fn path_of(block: &str) -> Option<&str> {
    let path = block.strip_prefix("rftoolsutility:")?;
    TICKING.contains(&path).then_some(path)
}

fn int(nbt: &Compound, key: &str, default: i32) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        Some(Tag::Byte(v)) => i32::from(*v),
        _ => default,
    }
}

fn set(nbt: &mut Compound, key: &str, value: i32) {
    nbt.insert(key.into(), Tag::Int(value));
}

fn set_flag(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], key: &str, value: &str) {
    if level.block_property(host, pos, key).as_deref() != Some(value) {
        host.set_block_property(pos, key, value);
    }
}

fn tick_timer(host: &mut dyn ServerHost, level: LevelRef, event: &mut BlockEntityTickEvent<'_>) {
    let nbt = &mut event.entity.nbt;
    let mut t = Timer {
        delay: int(nbt, "delay", 20),
        phase: int(nbt, "phase", 0),
        ..Timer::default()
    };
    let on = t.tick();
    set(nbt, "phase", t.phase);
    set_flag(host, level, event.pos, "powered", if on { "true" } else { "false" });
}

fn tick_counter(host: &mut dyn ServerHost, level: LevelRef, event: &mut BlockEntityTickEvent<'_>) {
    let nbt = &mut event.entity.nbt;
    let mut c = Counter {
        target: int(nbt, "target", 4),
        count: int(nbt, "count", 0),
        was_high: int(nbt, "was_high", 0) != 0,
        out_ticks: int(nbt, "out_ticks", 0),
    };
    let high = level.has_neighbor_signal(host, event.pos);
    let on = c.tick(high);
    let nbt = &mut event.entity.nbt;
    set(nbt, "count", c.count);
    set(nbt, "was_high", i32::from(c.was_high));
    set(nbt, "out_ticks", c.out_ticks);
    set_flag(host, level, event.pos, "powered", if on { "true" } else { "false" });
}

fn tick_transmitter(host: &mut dyn ServerHost, level: LevelRef, event: &mut BlockEntityTickEvent<'_>) {
    let now = level.game_time(host).unwrap_or(0);
    let signal = level.best_neighbor_signal(host, event.pos).unwrap_or(0);
    let channel = int(&event.entity.nbt, "channel", 0);
    CHANNELS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .report(host.dimension(), event.pos, channel, signal, now);
    *event.reply = BlockEntityTickReply::Unchanged;
}

fn tick_receiver(host: &mut dyn ServerHost, level: LevelRef, event: &mut BlockEntityTickEvent<'_>) {
    let now = level.game_time(host).unwrap_or(0);
    let channel = int(&event.entity.nbt, "channel", 0);
    let heard = CHANNELS.lock().unwrap_or_else(|e| e.into_inner()).level(channel, now);
    set_flag(host, level, event.pos, "power", &heard.clamp(0, 15).to_string());
    *event.reply = BlockEntityTickReply::Unchanged;
}

fn tick_environment(host: &mut dyn ServerHost, level: LevelRef, event: &mut BlockEntityTickEvent<'_>) {
    let phase = (int(&event.entity.nbt, "phase", 0) + 1) % env::PERIOD;
    set(&mut event.entity.nbt, "phase", phase);
    if phase != 0 {
        return;
    }
    let radius = int(&event.entity.nbt, "radius", env::DEFAULT_RADIUS);
    let modules: Vec<&env::Module> = event
        .entity
        .items
        .iter()
        .flatten()
        .filter_map(|s| env::module_of(s.item().id(host).strip_prefix("rftoolsutility:")?))
        .collect();
    if modules.is_empty() {
        return;
    }
    let cost = env::cost_per_tick(modules.iter().copied(), radius) * i64::from(env::PERIOD);
    let stored = i64::from(int(&event.entity.nbt, "energy", 0));
    if stored < cost {
        return;
    }
    set(&mut event.entity.nbt, "energy", (stored - cost) as i32);
    let Some(players) = level.online_players(host) else { return };
    for player in players {
        let Some(at) = player.entity().position(host) else { continue };
        if !env::in_range(event.pos, at, radius) {
            continue;
        }
        let Some(living) = host.living(player.entity()) else { continue };
        for m in &modules {
            if let Some(key) = host.effect_key(m.effect) {
                let effect = ActiveEffect::new(key, env::EFFECT_TICKS, m.amplifier, true, false, true);
                living.add_effect(host, &effect);
            }
        }
    }
}

struct Machines;

#[mod_api::horizon_part]
impl BlockPart for Machines {
    /// A click steps the block's setting and shows it.
    fn use_without_item(&self, host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
        let level = host.level(host.dimension())?;
        let pos = event.click.pos;
        let block = level.block(host, pos)?.id(host);
        let path = path_of(&block)?;
        let entity = host.block_entity(level, pos)?;
        let nbt = entity.data(host)?;
        let (key, value, message) = match path {
            "timer" => ("delay", logic::next_step(&logic::DELAYS, int(&nbt, "delay", 20)), "rftoolsutility.timer"),
            "counter" => ("target", logic::next_step(&logic::TARGETS, int(&nbt, "target", 4)), "rftoolsutility.counter"),
            "environmental_controller" => ("radius", logic::next_step(&env::RADII, int(&nbt, "radius", env::DEFAULT_RADIUS)), "rftoolsutility.radius"),
            _ => ("channel", (int(&nbt, "channel", 0) + 1) % 16, "rftoolsutility.channel"),
        };
        let mut patch = Compound::new();
        set(&mut patch, key, value);
        let _ = entity.merge_data(host, patch);
        if let Some(p) = host.player(event.click.player) {
            let _ = p.action_bar(host, Text::translate(message, vec![Text::literal(&value.to_string())]));
        }
        Some(InteractionResult::SuccessServer)
    }

    fn is_signal_source(&self, level: &dyn LevelHost, event: &mut IsSignalSourceEvent) {
        if matches!(
            level.state_block(event.state),
            Some("rftoolsutility:timer" | "rftoolsutility:counter" | "rftoolsutility:redstone_receiver")
        ) {
            event.source = true;
        }
    }

    fn block_signal(&self, level: &dyn LevelHost, event: &mut BlockSignalEvent) {
        let Some(block) = level.state_block(event.state) else { return };
        let Some(state) = level.state_string(event.state) else { return };
        match block {
            "rftoolsutility:timer" | "rftoolsutility:counter" if state.contains("powered=true") => event.signal = 15,
            "rftoolsutility:redstone_receiver" => {
                if let Some(i) = state.find("power=") {
                    let digits: String = state[i + 6..].chars().take_while(char::is_ascii_digit).collect();
                    event.signal = digits.parse().unwrap_or(0);
                }
            }
            _ => {}
        }
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for Machines {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        path_of(event.block)?;
        Some(Compound::new())
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if path_of(event.block).is_some() {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let Some(level) = host.level(host.dimension()) else { return };
        let Some(block) = level.block(host, event.pos).map(|b| b.id(host)) else { return };
        match path_of(&block) {
            Some("timer") => tick_timer(host, level, event),
            Some("counter") => tick_counter(host, level, event),
            Some("redstone_transmitter") => tick_transmitter(host, level, event),
            Some("redstone_receiver") => tick_receiver(host, level, event),
            Some("environmental_controller") => tick_environment(host, level, event),
            _ => {}
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for path in TICKING {
            let id = format!("{NS}:{path}");
            let _ = r.block_key(&id);
            let kind = r.block_entity_type(path).blocks(&[id.as_str()]).build();
            let _ = r.block_entity_part(&kind, Machines);
        }
        let controller = r.block_key("rftoolsutility:environmental_controller");
        r.block_energy(&controller, ModEnergyBuffer {
            capacity: 1_000_000,
            max_insert: 20_000,
            max_extract: 0,
            insert_faces: [true; 6],
            extract_faces: [false; 6],
        });
        let all: Vec<usize> = (0..MODULE_SLOTS).collect();
        r.block_container(&controller, ModContainer {
            slots: MODULE_SLOTS,
            faces: std::array::from_fn(|_| all.clone()),
            inputs: all.clone(),
            outputs: all,
            menu: Some("minecraft:generic_9x1".into()),
        });
    }
}
