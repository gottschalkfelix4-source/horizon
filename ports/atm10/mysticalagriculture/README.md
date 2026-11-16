# Mystical Agriculture (Horizon port)

Port of Mystical Agriculture 8.0.28 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's
observed behaviour and public description, not from its code. Sixteen crops and the five farmland
tiers are ported; the mod's hundred-odd crops, its crafting chain and its tools are not.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| 16 crops: inferium, prudentium, tertium, imperium, supremium, earth, air, water, fire, stone, coal, iron, gold, redstone, diamond, emerald | `<name>_crop` (wheat template), `<name>_seeds` (plant it), `<name>_essence`; the loot table gives the seeds back and 1 to 2 essence when fully grown (age 7) | ported, tiers assumed |
| Farmland tiers 1 to 5 (`inferium_farmland` to `supremium_farmland`) | Farmland-template blocks; a crop's random tick (its growth) is cancelled unless the farmland below is of its tier or higher; vanilla farmland is tier 0 and grows none | ported |
| Seed recipes, essence crafting (prosperity and inferium shards, tiers up), farmland recipes, infusion altar, ritual, seed reprocessor, soul extractor, cognizant dust, fertilizer, growth accelerators, watering can, mob-soul crops, the other crops | | not yet |

## Needs from Horizon

1. Crops growing on a mod farmland: the port uses the wheat template's own growth and survival rules; vanilla's crop blocks check for farmland by tag or block. If the crops pop off a mod farmland in game, the engine needs a way to put a block in `minecraft:supports_crops`-like survival (or a `can_survive` answer from a part).
2. A data-driven crop type (`mysticalagriculture:crop` with name, tier, ingredient, colours) so the 100+ crops are data, not code; the port generates its 16 by script.
3. Tinted item and block models (each essence and seed has its own colour in the original).

## To check against the NeoForge original

1. Tier of each crop and the essence amounts per harvest (the port gives 1 to 2 at full age).
2. Whether farmland tier gating is `>=` (assumed) and whether inferium crops grow on vanilla farmland.
3. Seed drop rules (the port always gives the seed back on any harvest and break).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
