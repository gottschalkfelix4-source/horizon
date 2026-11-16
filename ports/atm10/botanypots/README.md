# BotanyPots (Horizon port)

Port of BotanyPots 21.1.44 (ATM10 8.x, LGPL-2.1-only, so this port is too) as a native Horizon mod.
Written from the mod's observed behaviour and public description, not from its code. The soils and
crops are a table in the port; the original reads them from data packs.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| `botanypots:botany_pot` | A flower-pot-template block with nine slots in vanilla's 9x1 menu: soil (0), seed (1), seven output slots; hoppers and pipes insert soil and seed and take the harvest | ported |
| Growing | While soil and seed match, counts ticks (2400 for most crops, 3000 for melon, pumpkin and nether wart, 4800 for a sapling), less by the soil's bonus (farmland 10 %, grass, podzol and mycelium 5 %); no match, or taking the soil, resets it | ported, numbers assumed |
| Soils | Dirt, grass block, podzol, farmland, sand, red sand, soul sand, mycelium, end stone, each with the categories crops need | ported (9 of them) |
| Crops | Wheat, carrot, potato, beetroot, melon, pumpkin, sugar cane, cactus, nether wart, oak sapling, red mushroom with their drops, counts and chances | ported (11 of them) |
| Harvest | When ripe, adds every drop to the output slots if all fit, else waits for room; seeds regrow at once | ported |
| Data-driven soils and crops, the other tree types and crops, fertilizer and growth-rate items, hopper-like pot behaviour, the pot's colour variants, rendering the soil and plant in the pot, the Mystical add-on | | not yet |

## Needs from Horizon

1. A way to read data-pack entries from a mod (`datapack_entries` and `datapack_read` exist on `LevelRef`): the port could then load `botanypots:soil` and `botanypots:crop` files as the original does; it keeps a table instead.
2. A block entity renderer that draws the soil block and the growing plant inside the pot.
3. Mod menu screens (the vanilla 9x1 menu has no soil/seed labels).

## To check against the NeoForge original

1. Growth times and the soil bonuses; whether a bonus is a speed-up or a flat tick count.
2. The drop lists and chances per crop.
3. Whether the pot harvests automatically or waits to be right-clicked, and its slot layout.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
