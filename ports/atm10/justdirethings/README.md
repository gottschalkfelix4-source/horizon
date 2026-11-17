# Just Dire Things (Horizon port)

Port of Just Dire Things 1.5.7 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's data files
(item and block names, recipes, loot tables, tags, the furnace fuel data map; `gen_data.py` upgrades them to
26.3's formats with `port_convert.py`, ingredients as `"mod:item"` and `"#tag"`) and its observed behaviour,
never from its code. Built and unit-tested only; not run in the game.

| Part | Behaviour | Status |
|---|---|---|
| Goo blocks (Primogel, Blazebloom, VoidShimmer, Shadowpulse: tiers 1 to 4) | Turn an adjacent block into a raw ore by the original's `goospread` recipes: iron block to raw ferricore (tier 1, 2400 ticks), gold block to raw blazegold (tier 2), diamond block to raw celestigem (tier 3, 4800), netherite block to raw eclipse alloy (tier 4), coal blocks into the four raw coals; the tier must meet the recipe's; the work restarts when the block goes | ported (the original's exact spread rule is assumed: one neighbour at a time) |
| Simple Coal Generator | Five fuel slots (hopper menu, hoppers work), burns each fuel for its burn time at 40 FE per tick, pushes up to 1,000 per tick to neighbours | ported; the number is assumed |
| Fuels: Primal Coal, Blaze Ember, Voidflame Coal, Eclipse Ember, their blocks and Charcoal Block | `minecraft:cooking_fuel` with the original's burn times (4,800 to 1,296,000 ticks) | ported, from its data map |
| Ferricore, Blazegold, Celestigem, Eclipse Alloy: sword, pickaxe, axe, shovel, hoe, helmet, chestplate, leggings, boots; Celestigem and Eclipse Alloy paxels; four bows | Items with durability, attack, mining speed and tier rules and armor values of their own | declared; every number is an assumption (the original sets them in code), looks are iron's |
| Ingots, raw ores, blocks, templates, 50 upgrades, wands, canisters, Time Crystal and its blocks and clusters, 21 machine blocks (breakers, placers, clickers, droppers, sensors, swappers, fluid placers and collectors, energy transmitter, experience holder, item collector, inventory holder, player accessor, paradox machine, eclipse gate), Portal Gun, Creature Catcher and the rest | Declared with vanilla templates | data only, no behaviour |
| Recipes | 227 of the original's: crafting, smelting and blasting, smithing (tool tiers), the goo recipes | carried |

## Not ported

1. All machines' behaviour except the generator, tool abilities (the 151 `ability` recipes put upgrades on
   tools: Tree Feller, Ore Miner, Smelter, Hammer, Flight, Elytra and the rest) and the `paxel` recipe type;
   the engine has no per-item upgrade state a tool could read in a mining hook beyond a mod's own data.
2. Every fluid (Polymorphic, Portal, Time, XP, the refined and unrefined fuels) and their buckets, so the
   `fluiddrop` recipes and the goo recipes that act on fluid blocks are left out; the `goospread_tag` recipe
   (charcoal blocks, an input by tag).
3. Time Crystal growth (the budding block), Gooey ore revive tags, Portal Gun, Time and Polymorphic wands.
4. The tags for entities and biomes are not carried.

## To check against the NeoForge original

Tool and armor numbers (`MATS` in `gen_data.py`), the generator's output, the goo's rule, the machine blocks'
hardness.

## Build

```sh
sh ../dev-sdk.sh && cargo test --lib
python3 gen_data.py <folder with the unpacked data of the original's jar>
```
