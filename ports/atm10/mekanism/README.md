# Mekanism (Horizon port)

Port of [Mekanism](https://github.com/mekanism/Mekanism) 10.7 (ATM10 8.x, MIT) as a native Horizon
mod. Written from the mod's observed behaviour and public description, not from its code.
Mekanism is a large mod (Core, Generators, Tools, Additions); this is the first slice, built so
the later ones can sit on it.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| Ores and metals | Osmium, tin, lead, uranium, fluorite ores (stone and deepslate), raw ores, ingots, nuggets, dusts, clumps, shards, crystals, dirty dusts, storage blocks, steel, bronze, refined glowstone/obsidian, enriched items, alloys, circuits | blocks and items declared (placeholders for look and numbers until `carry`); no world generation, loot tables or crafting recipes yet |
| Energy cubes | Four tiers (2, 8, 32, 128 MJ at 2.5 J per FE); push into adjacent machines, cables draw from them | ported, numbers to check |
| Universal cables | Four tiers; a network (up to 4096 cables) run by its lowest cable moves energy from cubes it touches to machines it touches, at the lowest tier's rate | ported, network model is the port's own (Mekanism's transmitters hold a buffer and balance) |
| Enrichment chamber | 50 J/tick, 200 ticks: ores to two dusts, fluorite ore to six gems, redstone/coal/diamond/glowstone/iron dust to enriched items | ported, recipe table hard-coded |
| Crusher | Ingots to dusts, cobblestone to gravel to sand, bones, blaze rods, gems to dusts | ported, recipe table hard-coded |
| Energized smelter | The engine's `minecraft:smelting` recipes, same power and time | ported |
| Precision sawmill | Logs and stems to six planks (no sawdust chance yet) | ported |
| Combiner | Dust and cobblestone into the metal's ore | ported |
| Metallurgic infuser | Infusing items in the second slot fill an infuse tank (10 per item, 1000 max); makes infused/reinforced/atomic alloys, steel and bronze dust | ported, recipe table hard-coded |
| `mekanism:chemicals` | `CapabilityKey` over `ResourceHandler<ChemicalResource>`, a one-tank block handler saved in the block entity | declared and unit-tested; no machine registers it yet |
| Menus | Vanilla hopper menu (slots: input, output, energy item, two upgrades) | stand-in until mod menus with energy bars exist |
| Generators, tools, additions | in the [Generators](../mekanismgenerators/), [Tools](../mekanismtools/) and [Additions](../mekanismadditions/) ports | | |
| Factories, tiers of machines, upgrades, side configuration, security, QIO, teleporters, digital miner, multiblocks (generators, turbine, fission, fusion, SPS), tools and armor, Additions, gas machines, pipes (pressurized, logistical, mechanical, thermodynamic), jetpack/mekasuit, chemical/fluid processing | | not yet |

## Needs from Horizon

Reported to the API thread, which builds them (see `../ENGINE-GAPS.md`):

1. Mod menu widgets (energy bar, progress arrow, number fields) for machine screens.
2. Per-slot rules for a container so hoppers do not push into the output or the energy slot.
3. Item-backed container menus (the portable QIO dashboard, the configurator, the Mekasuit).
4. `ChunkRef::block_entities`, for multiblocks and the digital miner scanning a chunk.
5. A data-driven recipe type with several inputs, fluids and chemicals so `mekanism:enriching`,
   `:crushing`, `:infusing`, `:dissolution` and the rest come from datapacks (as `LevelRef::recipe_for` does
   for crafting kinds); until then the tables are in `src/rules.rs`.

Not a gap: gases, infuse types, pigments and slurries need no engine change (`CapabilityKey::new`).

## To check against the NeoForge original

1. Every number in `src/rules.rs` (cube capacity and rate, cable rate, machine power, time and buffer).
2. Whether raw ores enrich to two dusts and ores to two (as written), and the fluorite yield.
3. The cube's face configuration (here: all faces in and out).
4. Hardness and resistance (`gen_data.py` writes placeholders until `horizon-port-pack carry`).

## Build

```sh
sh ../dev-sdk.sh                              # once: links the checkout's mod API
cargo test                                    # in this folder
python3 gen_data.py                           # rewrites data/ (placeholders)
horizon-launcher native-build . --instance <instance> --trust-native-code
```
