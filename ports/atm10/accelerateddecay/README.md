# Accelerated Decay (Horizon port)

Port of [Accelerated Decay](https://modrinth.com/mod/accelerated-decay) 21.0.0 (ATM10 8.x,
LGPL-3.0-only, by ErrorMikey) as a native Horizon mod. Written from the mod's public
description and changelog, not from its code. This port is released under the original's
licence, **LGPL-3.0-only** (see `LICENSE-ORIGINAL.md`; `Cargo.toml` `license`).

## What it does

| Behaviour | Status |
|---|---|
| A leaf that becomes a decaying leaf (`distance=7`, `persistent=false`, in `#minecraft:leaves`) is queued; each server tick breaks up to 5 queued leaves, so a felled tree's crown clears in seconds | ported, not yet checked against the original |
| Player-placed (persistent) leaves never decay | holds (vanilla's rule, checked again before breaking) |
| Breaking goes through vanilla's leaf random tick: same drops (saplings, sticks, apples), `leaves_decay` hook (another mod can still keep a leaf), level events | ported |
| The queue survives a restart | not yet: kept in memory. Leaves left over still decay by vanilla's random ticks |

How: vanilla leaves update their `distance` with a scheduled tick when a neighbour changes, so
breaking a log ripples a `setBlock` through the crown. `block_changed` sees each leaf that turns
into a decaying one; `server_tick` breaks the queue's oldest five, after checking each is still a
decaying leaf (a player may have broken it, or a new log reconnected it).

## Needs from Horizon

- To keep the queue across restarts as the newer original does ("saved in world data"): nothing
  missing, `ServerHost::world_data_files()` or a level attachment would carry it; left for a
  later version of this port.

## To check against the NeoForge original (oracle on a machine with the jar)

1. 21.0.0's method: the changelog of later versions says early versions scanned for leaves when a
   log was broken and broke them "as a player"; later ones queue leaf updates and break 5 per
   tick. This port follows the later description. Check 21.0.0's rate, its scan radius, and
   whether its drops are a player's (tool enchantments, statistics) or a plain decay's.
2. Whether modded leaves outside `#minecraft:leaves`, or without a `distance` property, are
   handled.
3. Whether the decay is spread with a random delay or strictly in order.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
