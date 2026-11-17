# Mystical Agradditions (Horizon port)

Port of Mystical Agradditions 8.0.15 (ATM10 8.x, MIT) as a native Horizon mod, beside the
[Mystical Agriculture port](../mysticalagriculture/). Written from the mod's data files
(recipes, loot tables, tags, worldgen, translations; `gen_data.py` upgrades them to 26.3's formats
with `port_convert.py`) and its observed behaviour, never from its code. Not run in the game.

| Part | Status |
|---|---|
| Insanium farmland (tier 6) and six awakened crops (nether star, dragon egg, awakened draconium, gaia spirit, neutronium, nitro crystal): seeds, essence, crop block, loot (seed back, 1 to 2 essence at age 7) | ported; a crop grows only on tier 6 farmland (`src/rules.rs`) |
| Insanium essence, ingot, nugget, gemstone and their blocks; five coals and insanium coal with blocks; six apples; dragon scale, dragon egg chunk, nether star shard, withering soul, creative essence; six crux blocks | declared (data, vanilla templates for look); crafting recipes of vanilla types carried from the original |
| Six paxels (inferium to awakened supremium) | items with the tool rules of pickaxe, axe and shovel; numbers (durability, speed, tier, attack) from the mod's own Tinkers' head stats, the awakened one assumed at 1.25x supremium |
| End and nether inferium ore | blocks, loot (essence with fortune, silk touch) and worldgen (biome modifiers in the end and nether) from the original's data |

## Differences and what is missing

1. The awakened crops' items live under `mysticalagradditions:` (the original registers them under
   `mysticalagriculture:`; a content mod owns only its own namespace). Recipes are rewritten to match.
2. Not carried: prosperity ores (they drop Mystical Agriculture's prosperity shard, not in its port), the
   seed recipes and infusion/awakening/reprocessor/farmland-till recipes (types of Mystical Agriculture's
   machines, which the port lacks), Tinkers' Construct recipes and materials, recipes that need Mystical
   Agriculture's ingots, tools, prosperity or infusion crystals, molten metal fluids and buckets, Soul
   Siphoner trait. The recipes that need botania, powah or similar stay behind `neoforge:mod_loaded`.
3. Apples use golden apple (tiers 1 and 2) and enchanted golden apple (tiers 3 to 6) as look and effect
   stand-ins; the original's effects are not known to the port.
4. Coal burn times: the coals are fuel in the original, but its data has no value for them (set in code), so they burn as plain coal does; a `minecraft:cooking_fuel` component would carry them.
5. Paxel extras (stripping, flattening, tilling) belong to the original's item class and do not follow a
   template item.

## To check against the NeoForge original

Tier 6 for the awakened crops, the apples' effects, the paxel numbers, the blocks' hardness (taken as
iron's and coal's), the ore size and height from the data against what generates in game.

## Build

```sh
sh ../dev-sdk.sh && cargo test --lib
python3 gen_data.py <folder with the unpacked data of the original's jar>   # rewrites data/ and assets/
```
