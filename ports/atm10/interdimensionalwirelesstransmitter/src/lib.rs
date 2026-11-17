//! Interdimensional Wireless Transmitter for Horizon: redstone over any distance and dimension.
//!
//! Written from the mod's public description, never from its code (docs/modpacks.md,
//! "Licences"). README.md says how little of the original's behaviour could be recalled, and what
//! a check against it has to confirm.
//!
//! A transmitter reads the strongest redstone signal at its sides each tick and says it on its
//! frequency (0 to 15, saved as `frequency`, stepped by using the block). A receiver on the same
//! frequency, anywhere in any dimension, gives the strongest signal it hears, as the block's
//! `power` state. What is said lives in this process only (the shared `Air`), not in the world:
//! transmitters repeat it every tick, so nothing needs saving.
use std::sync::{LazyLock, Mutex};

use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, BlockSignalEvent, Compound, Hooks, Host, InteractionResult,
    IsSignalSourceEvent, LevelHost, LevelSide, Registrar, RegistrationContext, ServerHost, Tag,
    UseWithoutItemEvent,
};

pub mod rules;
use rules::*;

const TRANSMITTER: &str = "interdimensionalwirelesstransmitter:transmitter";
const RECEIVER: &str = "interdimensionalwirelesstransmitter:receiver";

static AIR: LazyLock<Mutex<Air>> = LazyLock::new(|| Mutex::new(Air::default()));

fn frequency(nbt: &Compound) -> i32 {
    match nbt.get(&"frequency".into()) {
        Some(Tag::Int(v)) => v.rem_euclid(FREQUENCIES),
        _ => 0,
    }
}

struct Wireless;

#[mod_api::horizon_part]
impl BlockPart for Wireless {
    /// Using a transmitter or receiver steps its frequency up by one (down while sneaking).
    fn use_without_item(&self, host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
        let level = host.level(host.dimension())?;
        let pos = event.click.pos;
        let id = level.block(&*host, pos)?.id(&*host);
        if id != TRANSMITTER && id != RECEIVER {
            return None;
        }
        let player = host.player(event.click.player)?;
        let down = player.entity().is_shift_key_down(&*host).unwrap_or(false);
        let entity = host.block_entity(level, pos)?;
        let next = (frequency(&entity.data(&*host)?) + if down { -1 } else { 1 }).rem_euclid(FREQUENCIES);
        let mut patch = Compound::new();
        patch.insert("frequency".into(), Tag::Int(next));
        let _ = entity.merge_data(host, patch);
        let line = mod_api::Text::translate(
            "message.interdimensionalwirelesstransmitter.frequency",
            vec![mod_api::Text::literal(next.to_string())],
        );
        let _ = player.action_bar(host, line);
        Some(InteractionResult::SuccessServer)
    }

    fn block_signal(&self, level: &dyn LevelHost, event: &mut BlockSignalEvent) {
        let Some(text) = level.state_string(event.state) else { return };
        let power = text
            .split("power=")
            .nth(1)
            .and_then(|rest| rest.split([',', ']']).next())
            .and_then(|p| p.parse::<i32>().ok());
        if let Some(power) = power {
            event.signal = power;
        }
    }

    fn is_signal_source(&self, level: &dyn LevelHost, event: &mut IsSignalSourceEvent) {
        event.source = level.state_string(event.state).is_some_and(|s| s.contains("power="));
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for Wireless {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == TRANSMITTER || event.block == RECEIVER).then(|| {
            let mut nbt = Compound::new();
            nbt.insert("frequency".into(), Tag::Int(0));
            nbt
        })
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == TRANSMITTER || event.block == RECEIVER {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        *event.reply = BlockEntityTickReply::Unchanged;
        let pos = event.pos;
        let Some(level) = host.level(host.dimension()) else { return };
        let Some(now) = level.game_time(&*host) else { return };
        let Some(id) = level.block(&*host, pos).map(|k| k.id(&*host)) else { return };
        let freq = frequency(&event.entity.nbt);
        if id == TRANSMITTER {
            let strength = level.best_neighbor_signal(&*host, pos).unwrap_or(0);
            if let Ok(mut air) = AIR.lock() {
                air.say(freq, (host.dimension().to_owned(), pos), strength, now);
            }
        } else if id == RECEIVER {
            let heard = AIR.lock().map_or(0, |air| air.hear(freq, now));
            let current = level
                .block_property(&*host, pos, "power")
                .and_then(|p| p.parse::<i32>().ok())
                .unwrap_or(0);
            if heard != current {
                // setting the property updates the neighbours, which read the new signal
                host.set_block_property(pos, "power", &heard.to_string());
            }
        }
        if now % 200 == 0 {
            if let Ok(mut air) = AIR.lock() {
                air.forget(now);
            }
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let kind = r.block_entity_type("wireless").blocks(&[TRANSMITTER, RECEIVER]).build();
        let _ = r.block_entity_part(&kind, Wireless);
    }
}
