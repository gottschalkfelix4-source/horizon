# Laser Bridges (Horizon port)

Port of Laser Bridges 6.2 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's public
description, not from its code. I could not recall the original's mechanics precisely, so this
port is the simplest reading of "emitter, bridge, receiver": treat every row below as an assumption.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| `laser_emitter` | Dropper-template block (`facing`). While powered by redstone it fills the air in front of it with beam blocks up to 32 long, stopping at the first block that is not air; unpowered, it removes them. Checked every 4 ticks | ported (assumed) |
| `laser_beam` | Glass-template block (`axis`), light 7, no item. Removes itself on a random tick when no emitter faces it along its axis | ported (assumed) |
| `laser_receiver` | Lights (`lit`) when a beam ends on it and gives a redstone signal of 15 | ported (assumed) |
| Colours, energy use, entity damage or boosting, lenses, a bridge through other emitters | | not yet |

## Needs from Horizon

1. A way to react to the removal of a block with level writes (`affect_neighbors_after_removal` only reads): breaking an emitter cannot clear its bridge at once, so beams clean themselves on a random tick (a minute or so).
2. A way for a block entity tick to schedule itself (a delayed tick) instead of every tick check.

## To check against the NeoForge original

1. What the mod's blocks are (emitter, receiver, bridge) and what drives the emitter: redstone, energy, or both.
2. The range, whether the bridge is solid for players and mobs, and whether it can pass through entities.
3. Whether the receiver gives redstone, or relays the bridge onward.
4. Whether the beam replaces non-air blocks such as water, plants or snow layers.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
