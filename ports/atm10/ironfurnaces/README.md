# Iron Furnaces (Horizon port)

Port of [Iron Furnaces](https://github.com/Qelifern/IronFurnaces) 4.3.2 (ATM10 8.x, Apache-2.0) as a
native Horizon mod. Written from the mod's observed behaviour and public description, not from its code.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| 13 furnace blocks (`ironfurnaces:<tier>_furnace`, copper to unobtainium and the million furnace) | Vanilla furnace template (`facing`, `lit`), one block entity type `ironfurnaces:furnace` | ported |
| Smelting | Vanilla's furnace tick with the tier's cook time; `minecraft:smelting` recipes through the engine's matcher | ported, not yet checked against the original |
| Fuel | The fuel's burn time scaled by cook time / 200, so one coal smelts eight items on every tier | assumed, to check |
| Menu | Vanilla's furnace menu (3 slots, 4 data slots); hoppers as on a vanilla furnace | ported |
| Upgrades (tier kits), augments (blasting, smoking, speed, fuel efficiency, factory, generator), redstone modes, auto-io, XP | | not yet |
| Million furnace's rainbow effects, crystal glass look | | not yet (client) |

## To check against the NeoForge original

1. Default cook times per tier (`TIERS` in `src/lib.rs`, from the default server config as remembered).
2. Whether fuel burns for the same number of items on every tier, or for the same ticks.
3. Hardness and blast resistance of each tier; `data/ironfurnaces/horizon/block/*.json` are placeholders
   until `horizon-port-pack carry` reads them (and the recipes, models, loot tables) from the jar.
4. Experience stored per recipe and paid out on taking the result (not ported yet).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
