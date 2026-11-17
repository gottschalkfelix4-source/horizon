# Comforts (Horizon port)

Port of [Comforts](https://github.com/TheIllusiveC4/Comforts) 9.0.5 (ATM10, LGPL-3.0-or-later) as
a native Horizon mod, under the same licence. Written from the mod's observed behaviour and public
description, not from its code.

## What it does

| Feature | Behaviour | Status |
|---|---|---|
| 16 sleeping bags `comforts:sleeping_bag_<colour>` | Two-part beds (bed templates), 3 pixels high | ported (placeholder block facts) |
| Sleeping bag: no spawn point | Sleeping in one keeps the player's old respawn point (`player_set_spawn` cancelled; `/spawnpoint` is left alone) | ported, not yet checked against the original |
| 16 hammocks `comforts:hammock_<colour>` | Two-part beds | ported (placeholder block facts, bed shape) |
| Hammock: sleep by day | Allowed outside vanilla's night window (time of day 12542 to 23459), refused at night with its own message; vanilla's other refusals (monsters near, too far, obstructed) stay | ported, not yet checked against the original |
| Hammock: stays asleep by day, wakes at night | `can_continue_sleeping` | ported |
| Hammock: wake at dusk | When a level's sleepers skip the day and one of them is in a hammock, the clock goes to the next dusk (time of day 13000) instead of the morning | ported, dusk time assumed |
| Hammock: no spawn point | as sleeping bags | ported |
| `comforts:rope_and_nail` | A wall block hammocks hang from | block only (placeholder on the tripwire hook template); the hanging rule is not yet |
| Hammock placement between two rope and nails | | not yet |
| Config (sleeping bag breakage chance, debuffs on waking, auto-use, allowed times) | | not yet: fixed to the behaviour above |
| Sleeping bag: item placed and slept in in one click | | not yet |
| Recipes, textures, models | | from `horizon-port-pack carry` on the jar |

## Needs from Horizon

- Nothing found missing for the ported part. The hammock's hanging rule needs a placement check
  (`can_survive` / `state_for_placement` parts exist; not attempted).

## To check against the NeoForge original (oracle on a machine with the jar)

1. The hammock's hours: is "day" vanilla's complement of the bed night (as here), or
   `Level.isDay()` (sky darkness)? Can it be used during a daytime thunderstorm?
2. The time a hammock's sleepers wake at (13000 here), and whether all sleepers must be in
   hammocks or one is enough (one here).
3. Whether 26.3's own sleep check (the dimension's bed rule) lets a day sleep finish at all; if
   not, `sleep_finished_time` never fires by day and the hammock skip needs an engine hook.
4. Whether sleeping bags and hammocks explode in the Nether and End as beds do (they do here,
   from the bed template).
5. Whether a sleeping bag also refuses to set the spawn on the first sleep after a bed's.
6. Block facts (hardness, shapes, sounds) from `horizon-port-pack carry`;
   `data/comforts/horizon/block/*.json` are placeholders.

## Licence

LGPL-3.0-or-later, as the original: `COPYING.LESSER` and `COPYING`. See `LICENSE-ORIGINAL.md`.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
