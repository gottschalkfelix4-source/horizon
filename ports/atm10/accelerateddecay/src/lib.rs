// SPDX-License-Identifier: LGPL-3.0-only
//! Accelerated Decay for Horizon: leaves that lose their last log decay within moments instead
//! of waiting for random ticks.
//!
//! Written from the mod's public description and changelog, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and what a harness against the NeoForge
//! original still has to confirm, is in README.md.
//!
//! How it works: vanilla leaves recompute their `distance` to the nearest log with a scheduled
//! tick whenever a neighbour changes, and a leaf that reaches 7 (and wasn't placed by a player)
//! only decays on a later random tick. `block_changed` sees each leaf that becomes such a
//! decaying leaf and queues it; every server tick breaks up to [`PER_TICK`] queued leaves with
//! vanilla's own random tick, so drops, the `leaves_decay` hook and level events stay vanilla's.
use std::collections::{HashSet, VecDeque};
use std::sync::Mutex;

use mod_api::{BlockChangedEvent, Hooks, LevelHost, ServerHost, ServerTickEvent};

/// How many queued leaves break each server tick.
pub const PER_TICK: usize = 5;

const LEAVES_TAG: &str = "minecraft:leaves";

/// Whether a block state written as `id[prop=value,...]` is a leaf that decays: `distance` 7
/// and not `persistent`. Blocks without a `distance` property (modded leaves of another
/// shape) never count.
pub fn decays(state: &str) -> bool {
    let Some(props) = state
        .split_once('[')
        .and_then(|(_, rest)| rest.strip_suffix(']'))
    else {
        return false;
    };
    let mut distance = None;
    let mut persistent = false;
    for prop in props.split(',') {
        match prop.split_once('=') {
            Some(("distance", v)) => distance = v.parse::<u8>().ok(),
            Some(("persistent", v)) => persistent = v == "true",
            _ => {}
        }
    }
    distance == Some(7) && !persistent
}

/// Leaves waiting to decay, oldest first, without repeats.
#[derive(Default)]
pub struct Queue {
    order: VecDeque<(String, [i32; 3])>,
    queued: HashSet<(String, [i32; 3])>,
}

impl Queue {
    pub fn push(&mut self, dimension: String, pos: [i32; 3]) {
        if self.queued.insert((dimension.clone(), pos)) {
            self.order.push_back((dimension, pos));
        }
    }

    /// Takes up to `n` leaves, oldest first.
    pub fn take(&mut self, n: usize) -> Vec<(String, [i32; 3])> {
        let n = n.min(self.order.len());
        let out: Vec<_> = self.order.drain(..n).collect();
        for entry in &out {
            self.queued.remove(entry);
        }
        out
    }
}

static QUEUE: Mutex<Option<Queue>> = Mutex::new(None);

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// A leaf just became a decaying one: queue it.
    fn block_changed(&self, level: &dyn LevelHost, event: &mut BlockChangedEvent) {
        if level.block_in_tag(event.new.block(), LEAVES_TAG) != Some(true) {
            return;
        }
        if !level.state_string(event.new).is_some_and(|s| decays(&s)) {
            return;
        }
        let dimension = event.dimension.key().to_string();
        if let Ok(mut queue) = QUEUE.lock() {
            queue
                .get_or_insert_with(Queue::default)
                .push(dimension, [event.pos.x, event.pos.y, event.pos.z]);
        }
    }

    fn server_tick(&self, host: &mut dyn ServerHost, _: &mut ServerTickEvent<'_>) {
        let due = match QUEUE.lock() {
            Ok(mut queue) => match queue.as_mut() {
                Some(q) => q.take(PER_TICK),
                None => return,
            },
            Err(_) => return,
        };
        for (dimension, pos) in due {
            let Some(level) = host.level(&dimension) else {
                continue;
            };
            // Still a decaying leaf? A player may have broken it, or a new log reconnected it.
            let still = level
                .block_state(host, pos)
                .and_then(|state| host.block_state_string(state))
                .is_some_and(|s| decays(&s));
            if still {
                level.block_random_tick(host, pos);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_unplaced_leaves_at_distance_seven_decay() {
        assert!(decays(
            "minecraft:oak_leaves[distance=7,persistent=false,waterlogged=false]"
        ));
        assert!(!decays(
            "minecraft:oak_leaves[distance=7,persistent=true,waterlogged=false]"
        ));
        assert!(!decays(
            "minecraft:oak_leaves[distance=6,persistent=false,waterlogged=false]"
        ));
        assert!(!decays("minecraft:stone"));
        assert!(!decays("mymod:odd_leaves[persistent=false]"));
    }

    #[test]
    fn the_queue_keeps_order_and_skips_repeats() {
        let mut q = Queue::default();
        q.push("minecraft:overworld".into(), [1, 2, 3]);
        q.push("minecraft:overworld".into(), [4, 5, 6]);
        q.push("minecraft:overworld".into(), [1, 2, 3]);
        q.push("minecraft:the_nether".into(), [1, 2, 3]);
        let first = q.take(2);
        assert_eq!(
            first,
            vec![
                ("minecraft:overworld".to_owned(), [1, 2, 3]),
                ("minecraft:overworld".to_owned(), [4, 5, 6]),
            ]
        );
        // Taken leaves may be queued again (a leaf that decays, regrows and decays).
        q.push("minecraft:overworld".into(), [1, 2, 3]);
        assert_eq!(q.take(5).len(), 2);
        assert!(q.take(5).is_empty());
    }
}
