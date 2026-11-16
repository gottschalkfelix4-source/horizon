//! Time in a Bottle for Horizon: a bottle that stores time while a player carries it, then
//! spends it to make a block tick faster: each use on the same block doubles its speed, up to
//! 256 times, for 30 seconds.
//!
//! Written from the mod's public description (its costs, durations, limits and command), never
//! from its code (docs/modpacks.md, "Licences"). What the port does, and what a harness against
//! the NeoForge original still has to confirm, is in README.md.
//!
//! How it works: the stored time is a component on the stack (`tiab:stored_time`, in ticks).
//! The first bottle in a player's inventory gains [`TICKS_CONST`] ticks each second. Using it on
//! a block records an acceleration (level, position, rate, ticks left); each server tick runs
//! the block's random tick as often as an accelerated chunk would pick it.
use std::sync::{Mutex, OnceLock};

use mod_api::{
    Command, CommandArgumentValue, CommandSource, Component, ComponentTypesEvent, Flow, Hand,
    Hooks, HostSide, HostTransaction, InteractionResult, InventoryTickEvent, ItemKey, ItemPart,
    ItemPlace, ItemStack, LevelRef, RegisterCommandsEvent, Registrar, RegistrationContext,
    RunModCommandEvent, ServerHost, ServerTickEvent, ServerTickPhase, SoundSource, Tag, Text,
    TooltipEvent, UseOnEvent,
};

/// The most time a bottle holds, in ticks (`max_stored_time`).
pub const MAX_STORED_TIME: i32 = 622_080_000;
/// Ticks gained each second (`ticks_const`).
pub const TICKS_CONST: i32 = 20;
/// Seconds one use lasts (`each_use_duration`).
pub const EACH_USE_DURATION: i32 = 30;
/// The highest rate exponent (`max_rate_multi`): 2^8 = 256 times.
pub const MAX_RATE_MULTI: u32 = 8;
/// Vanilla's chance that a random tick picks a given block: `randomTickSpeed` 3 picks per
/// 16x16x16 section, 1 in 4096 / 3.
const RANDOM_TICK_ONE_IN: u64 = 1365;

/// The stored time on a stack, in ticks.
pub struct StoredTime;

impl Component for StoredTime {
    type Value = i32;
    fn id(&self) -> &str {
        "tiab:stored_time"
    }
    fn decode(&self, value: &Tag) -> Option<i32> {
        match value {
            Tag::Int(v) => Some(*v),
            _ => None,
        }
    }
    fn encode(&self, value: i32) -> Tag {
        Tag::Int(value)
    }
}

fn stored(stack: &ItemStack) -> i32 {
    stack.get(&StoredTime).unwrap_or(0)
}

/// The time it costs to reach rate exponent `level` (1 is twice as fast), in ticks: 30 s for
/// x2 and x4, then doubling with the rate (x8 60 s, x16 120 s, ...).
pub fn cost(level: u32) -> i32 {
    let multiplier = 1i32 << level;
    EACH_USE_DURATION.max(EACH_USE_DURATION * multiplier / 4) * TICKS_CONST
}

/// What a use on a block does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    /// Raise the block to rate exponent `level`, spending `cost` ticks.
    Raise { level: u32, cost: i32 },
    /// Not enough time stored for the next step.
    TooLittleTime,
    /// Already at the highest rate.
    AtMax,
}

/// The next step for a block now at exponent `current` (0 when not accelerated), with
/// `stored` ticks in the bottle.
pub fn next_use(current: u32, stored: i32) -> Use {
    if current >= MAX_RATE_MULTI {
        return Use::AtMax;
    }
    let level = current + 1;
    let cost = cost(level);
    if stored < cost {
        Use::TooLittleTime
    } else {
        Use::Raise { level, cost }
    }
}

/// Where `Inventory.tick` reaches a slot: the 36 main slots, the armour, then the off hand.
pub fn slot_order(slot: i32) -> i32 {
    match slot {
        0..=35 => slot,
        100..=103 => 36 + slot - 100,
        _ => 40,
    }
}

/// Whether the bottle in `slot` is the one that gains time, given the slots that hold bottles.
pub fn gains_time(slot: i32, bottle_slots: &[i32]) -> bool {
    bottle_slots
        .iter()
        .all(|other| slot_order(*other) >= slot_order(slot))
}

/// `stored` after one second, capped.
pub fn after_one_second(stored: i32) -> i32 {
    stored.saturating_add(TICKS_CONST).min(MAX_STORED_TIME)
}

/// The slots `Inventory` saves, in its order.
const SLOTS: [i32; 41] = {
    let mut out = [0; 41];
    let mut i = 0;
    while i < 36 {
        out[i] = i as i32;
        i += 1;
    }
    out[36] = 100;
    out[37] = 101;
    out[38] = 102;
    out[39] = 103;
    out[40] = -106;
    out
};

/// One accelerated block.
#[derive(Clone, Debug, PartialEq)]
pub struct Acceleration {
    pub level: LevelRef,
    pub pos: [i32; 3],
    /// The rate exponent: 2^rate random-tick rolls per tick.
    pub rate: u32,
    pub ticks_left: i32,
}

static ACCELERATIONS: Mutex<Vec<Acceleration>> = Mutex::new(Vec::new());

/// The bottle's key, known once registration ran.
static BOTTLE: OnceLock<ItemKey> = OnceLock::new();

/// A small generator for the acceleration's rolls; nothing vanilla draws from it.
struct Rolls(u64);

impl Rolls {
    fn one_in(&mut self, n: u64) -> bool {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0.is_multiple_of(n)
    }
}

static ROLLS: Mutex<Rolls> = Mutex::new(Rolls(0x9E37_79B9_7F4A_7C15));

/// The note the bottle plays for exponent `level`: a rising scale.
fn pitch(level: u32) -> f32 {
    const SEMITONES: [i32; 8] = [-12, -10, -8, -7, -5, -3, -1, 0];
    let semitone = SEMITONES[(level.clamp(1, 8) - 1) as usize];
    2f32.powf(semitone as f32 / 12.0)
}

/// Changes the stack at `place` with `edit`, in a transaction of its own.
fn edit_stack(host: &mut dyn ServerHost, place: &ItemPlace, edit: impl FnOnce(&mut ItemStack)) {
    let _ = host.transaction(|host, tx| match place.edit(host, tx, edit) {
        true => Ok(()),
        false => Err(()),
    });
}

/// The bottle itself.
struct Bottle;

#[mod_api::horizon_part]
impl ItemPart for Bottle {
    /// Each second, the first bottle in the inventory gains time.
    fn inventory_tick(&self, host: &mut dyn ServerHost, event: &mut InventoryTickEvent<'_>) {
        let Some(time) = host
            .level("minecraft:overworld")
            .and_then(|l| l.game_time(host))
        else {
            return;
        };
        if time.rem_euclid(20) != 0 || stored(event.stack) >= MAX_STORED_TIME {
            return;
        }
        let Some(player) = host.players().acting_player() else {
            return;
        };
        let Some(bottle) = BOTTLE.get().copied() else {
            return;
        };
        let player = player.uuid();
        let bottles: Vec<i32> = SLOTS
            .into_iter()
            .filter(|slot| {
                ItemPlace::PlayerSlot {
                    player,
                    slot: *slot,
                }
                .stack(host)
                .is_some_and(|s| s.item() == bottle)
            })
            .collect();
        if !gains_time(event.slot, &bottles) {
            return;
        }
        let place = ItemPlace::PlayerSlot {
            player,
            slot: event.slot,
        };
        edit_stack(host, &place, |s| {
            let _ = s.set(&StoredTime, after_one_second(stored(s)));
        });
    }

    /// Used on a block (after the block's own use, so a furnace still opens unless sneaking):
    /// start or speed up its acceleration.
    fn use_on(&self, side: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = side else {
            return None;
        };
        let level = host.player(event.player)?.entity().level(host)?;
        if level
            .block_state(host, event.pos)
            .and_then(|s| host.block_state_string(s))
            .is_none_or(|s| s == "minecraft:air")
        {
            return None;
        }
        let mut accels = ACCELERATIONS.lock().ok()?;
        let at = accels
            .iter()
            .position(|a| a.level == level && a.pos == event.pos);
        let current = at.map_or(0, |i| accels[i].rate);
        let Use::Raise { level: next, cost } = next_use(current, stored(event.stack)) else {
            return Some(InteractionResult::Fail);
        };
        let place = ItemPlace::PlayerHand {
            player: event.player,
            hand: event.hand,
        };
        edit_stack(host, &place, |s| {
            let _ = s.set(&StoredTime, stored(s) - cost);
        });
        let accel = Acceleration {
            level,
            pos: event.pos,
            rate: next,
            ticks_left: EACH_USE_DURATION * TICKS_CONST,
        };
        match at {
            Some(i) => accels[i] = accel,
            None => accels.push(accel),
        }
        drop(accels);
        let centre = event.pos.map(|c| c as f64 + 0.5);
        level.play_sound(
            host,
            "minecraft:block.note_block.harp",
            centre,
            SoundSource::Blocks,
            0.5,
            pitch(next),
        );
        Some(InteractionResult::Success)
    }

    fn tooltip(&self, _: &dyn mod_api::ClientHost, event: &mut TooltipEvent<'_>) {
        let seconds = stored(event.stack) / TICKS_CONST;
        let args = [seconds / 3600, seconds / 60 % 60, seconds % 60]
            .map(|n| Text::literal(n.to_string()))
            .to_vec();
        event
            .lines
            .push((Text::translate("tooltip.tiab.stored_time", args), u32::MAX));
    }
}

/// One server tick of every acceleration: the rolls for each, then the clock.
fn tick_accelerations(host: &mut dyn ServerHost) {
    let mut accels = match ACCELERATIONS.lock() {
        Ok(mut a) if !a.is_empty() => std::mem::take(&mut *a),
        _ => return,
    };
    for a in &mut accels {
        let level = a.level;
        a.ticks_left -= 1;
        if !level.is_loaded(host, a.pos) {
            // An unloaded block (or a level closed since) waits; its time still runs out.
            continue;
        }
        let ticks = {
            let Ok(mut rolls) = ROLLS.lock() else {
                continue;
            };
            (0..1u32 << a.rate)
                .filter(|_| rolls.one_in(RANDOM_TICK_ONE_IN))
                .count()
        };
        for _ in 0..ticks {
            level.block_random_tick(host, a.pos);
        }
    }
    accels.retain(|a| a.ticks_left > 0);
    if let Ok(mut slot) = ACCELERATIONS.lock() {
        // Uses made during the tick (none expected: the tick runs no player action) stay.
        accels.append(&mut slot);
        *slot = accels;
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let bottle = r.item_key("tiab:time_in_a_bottle");
        let _ = BOTTLE.set(bottle);
        r.item_part(bottle, Bottle);
    }

    fn component_types(&self, _: &RegistrationContext, event: &mut ComponentTypesEvent<'_>) {
        event.types.push(StoredTime.id().to_owned());
    }

    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        if matches!(
            event.phase,
            ServerTickPhase::ServerTickEnd {
                world_running: true
            }
        ) {
            tick_accelerations(host);
        }
    }

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.extend([
            Command::op("tiab addtime <seconds:integer>", 2),
            Command::op("tiab removetime <seconds:integer>", 2),
        ]);
    }

    /// `/tiab addtime|removetime <seconds>` on the bottle in the player's hand.
    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        let sign = match event.command.literals.as_slice() {
            [a, b] if a == "tiab" && b == "addtime" => 1,
            [a, b] if a == "tiab" && b == "removetime" => -1,
            _ => return Flow::Continue,
        };
        let seconds = event.command.arguments.iter().find_map(|a| match a.value {
            CommandArgumentValue::Integer(n) if a.name == "seconds" => Some(n),
            _ => None,
        });
        let (CommandSource::Player { player, .. }, Some(seconds)) = (&event.source, seconds) else {
            return Flow::Cancel;
        };
        let player = *player;
        let bottle = BOTTLE.get().copied();
        let hand = [Hand::Main, Hand::Off].into_iter().find_map(|hand| {
            let place = ItemPlace::PlayerHand {
                player: player.uuid(),
                hand,
            };
            let stack = place.stack(host)?;
            (Some(stack.item()) == bottle).then_some((place, stored(&stack)))
        });
        let line = match hand {
            Some((place, now)) => {
                let delta = seconds.saturating_mul(TICKS_CONST).saturating_mul(sign);
                let new = now.saturating_add(delta).clamp(0, MAX_STORED_TIME);
                edit_stack(host, &place, |s| {
                    let _ = s.set(&StoredTime, new);
                });
                Text::translate(
                    "commands.tiab.stored",
                    vec![Text::literal((new / TICKS_CONST).to_string())],
                )
            }
            None => Text::translate("commands.tiab.no_bottle", vec![]),
        };
        let _ = player.tell(host, line);
        Flow::Cancel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn costs_follow_the_documented_table() {
        // x2 30 s, x4 30 s, x8 60 s, x16 120 s, x32 240 s, x64 480 s.
        let seconds: Vec<i32> = (1..=6).map(|l| cost(l) / 20).collect();
        assert_eq!(seconds, vec![30, 30, 60, 120, 240, 480]);
    }

    #[test]
    fn each_use_doubles_until_the_cap_and_needs_the_time() {
        assert_eq!(
            next_use(0, 600),
            Use::Raise {
                level: 1,
                cost: 600
            }
        );
        assert_eq!(next_use(0, 599), Use::TooLittleTime);
        assert_eq!(
            next_use(2, 10_000),
            Use::Raise {
                level: 3,
                cost: 1200
            }
        );
        assert_eq!(next_use(8, i32::MAX), Use::AtMax);
    }

    #[test]
    fn only_the_first_bottle_gains_and_it_stops_at_the_cap() {
        assert!(gains_time(3, &[3, 20, -106]));
        assert!(!gains_time(20, &[3, 20]));
        assert!(!gains_time(-106, &[-106, 102]));
        assert!(gains_time(102, &[-106, 102]));
        assert_eq!(after_one_second(100), 120);
        assert_eq!(after_one_second(MAX_STORED_TIME - 5), MAX_STORED_TIME);
    }
}
