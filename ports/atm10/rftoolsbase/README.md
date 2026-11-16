# RFToolsBase (Horizon port)

Port of RFToolsBase 1.21-6.0.11 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's
observed behaviour and public description, not from its code.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| Machine Frame, Machine Base | Crafting blocks, no behaviour (placeholder block files) | ported |
| Dimensional Shard ores (overworld, nether, end) and the shard | Blocks and item registered; worldgen and drops | not yet (needs loot tables from the pack carry) |
| Filter Module | Rule engine (`src/filter.rs`): whitelist/blacklist, match damage, components, tags, mod; settings saved in `custom_data`; `filter_of` / `facts_of` for the other RFTools ports | ported (rules); no editing screen |
| Information Screen | Reads the energy of any adjacent block through the `ENERGY` capability every `SAMPLE_TICKS`, keeps the line, using it cycles text / percentage / rate | ported, display is a data line only |
| Screen rendering the line on the block face, filter ghost-item GUI | | not yet (client) |

## Needs from Horizon

- A way to open a mod GUI with ghost (non-consuming) slots and toggle buttons, to edit a filter module in hand.
- A block entity renderer or text-on-block-face part for the Information Screen.
- A tag listing for an item (`tags_of(item)`): today the filter asks per tag of its ghost items.

## To check against the NeoForge original

1. The exact option set and defaults of the filter module (blacklist, damage, components, tags, mod).
2. Information Screen update interval and how rate is averaged (`src/info.rs`).
3. Whether the screen also reads other modules (items, fluids), not only energy.
4. Dimensional shard ore hardness, ore counts and drops (placeholders).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
