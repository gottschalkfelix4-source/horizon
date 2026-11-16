# Interdimensional Wireless Transmitter (Horizon port)

Port of Interdimensional Wireless Transmitter 1.21.1-0.1.5 (ATM10 8.x, MIT) as a native Horizon mod.
Written from the mod's public listing, not from its code. **I could not recall what the original
transmits precisely** (redstone, items, or something else): this port assumes redstone signals on
frequencies, across dimensions. Treat the whole port as a placeholder for the real behaviour.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| `transmitter` | Reads the strongest redstone signal at its sides each tick and says it on its frequency | assumed |
| `receiver` | Gives the strongest fresh signal heard on its frequency (state `power` 0 to 15, a redstone source) | assumed |
| Frequency | 0 to 15, saved as `frequency`; using the block steps it up, sneaking steps it down, shown on the action bar | assumed |
| Across dimensions | The shared state is in the process, not per level, so any dimension hears any other | ported |
| Words age out after 3 ticks so a removed transmitter goes silent | | ported |
| Ownership or teams, items/fluids/energy transfer, linking items | | not yet |

## Needs from Horizon

1. A world-wide saved-data holder (`Holder::Level` is per dimension) if frequencies ever need to survive a restart; today nothing is saved because transmitters repeat their word every tick.
2. A neighbour-changed hook with analog strength read, so a transmitter reacts at once instead of each tick (the port polls `best_neighbor_signal`).
3. Screens for typing a frequency (vanilla has no number field for a block entity).

## To check against the NeoForge original

1. What is transmitted (redstone, items, fluids, energy, entities?) and whether blocks or items carry the link.
2. Whether frequencies are numbers, colours or items, and whether they are private to a player or team.
3. Whether there is a range or an energy cost.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
