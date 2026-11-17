# Farmer's Delight (Horizon port)

Port of [Farmer's Delight](https://github.com/vectorwing/FarmersDelight) 1.3.4 (ATM10 8.x, MIT) as a
native Horizon mod. Written from the mod's observed behaviour and public description, not from its code.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| Cooking Pot (`farmersdelight:cooking_pot`) | 6 ingredient slots, meal slot, container slot, output; cooks `farmersdelight:cooking` recipes (shapeless, the data pack's own files) over a heat source in the recipe's `cookingtime`; a finished meal is served into its container (bowl) in the output slot, or by right-clicking the pot with the container | ported, not yet checked against the original |
| Heat sources | Block below in `farmersdelight:heat_sources`, else fire, soul fire, lava, magma, a lit campfire or lit stove | ported; heat conductors (a tray between pot and fire) not yet |
| Cooking Pot menu | Vanilla's `generic_3x3` (9 slots) in place of the mod's own screen; no progress arrow | stand-in |
| Stove | Placed lit; a shovel puts it out, flint and steel or a fire charge lights it; six grill spots take campfire-cookable items from the top and pop the result out above after the recipe's time; heats a pot or skillet on it | ported; burning entities that stand on it, slot by click position not yet |
| Skillet (placed) | Takes a whole stack of a campfire-cookable item; cooks one item at a time over a heat source and pops it out; empty hand takes the stack back | ported; skillet as a held weapon and in-hand cooking not yet |
| Cutting Board | Takes one item; the right tool (`farmersdelight:cutting` recipe's tool, tool actions mapped to vanilla tool tags) cuts it, results rolled by their `chance` and popped above; the tool loses one durability; empty hand takes the item back | ported |
| Rich Soil, Rich Soil Farmland | Random tick: the crop above grows one `age` stage with chance 0.2; a brown or red mushroom above turns into its colony | ported |
| Mushroom colonies | Grow `age` 0 to 3 with chance 0.2 per random tick, only on rich soil; they don't spread like a mushroom | ported; shearing a colony for mushrooms not yet |
| Organic Compost | `composting` 0 to 7, one stage per random tick with a chance raised by activators (mushrooms, mycelium, rich soil, compost, colonies), water and open sky; at the end it becomes rich soil | ported, chances assumed |
| Feasts (roast chicken, stuffed pumpkin, honey-glazed ham, shepherd's pie) | 4 `servings`; right-click with a bowl gives one serving; an empty feast is cleared (its loot table drops) | ported |
| Crops: cabbages, onions, tomatoes | Vanilla crop templates (wheat, carrots, beetroots); seeds and onion plant them | ported as plain crops; tomato vines on ropes, rice in water, wild crops not yet |
| Foods (meals, vegetables), knives | Templates with food and durability components | data only |
| Comfort and Nourishment effects, pies, rope, canvas, tray, baskets, cabinets, straw, tatami, signs | | not yet |

## Needs from Horizon

- A server-side mod menu that a Horizon client can draw without the mod (a declared screen layout for
  `data/<id>/horizon/menu/*.json`): the Cooking Pot's own screen (progress arrow, heated flame, meal
  preview) needs a client screen today, so the port uses `minecraft:generic_3x3` on a `side = "server"` mod.
- `LevelWriteHost::bone_meal(pos)` (or `perform_bonemeal` on the block above) inside `random_tick`: rich
  soil grows the plant above by one `age` stage instead of calling the plant's own `performBonemeal`, so
  saplings, mushrooms other than vanilla's two, and modded plants without `age` don't benefit.
- `LevelHost::block_in_tag` takes a `BlockKey`, but a random tick only has the neighbour's state; a
  `state_in_tag(state, tag)` read would let compost activators come from the `farmersdelight:compost_activators`
  tag instead of the built-in list.

## To check against the NeoForge original

1. Cooking Pot: whether progress falls back (2 ticks a tick, as a furnace) or resets when the heat or the
   recipe goes; whether remainders (a bucket from milk) stay in the slot or drop.
2. Stove and skillet cooking time: the campfire recipe's `cookingtime` is used unchanged; FD may speed the
   skillet up (Fire Aspect on the skillet item, config `skilletCookingTime`).
3. Rich soil: the 0.2 chance (config `richSoilBoostChance`) and that it applies the plant's bone meal growth
   (here one stage); which mushrooms become colonies; colony growth chance and light rules.
4. Organic compost: the chance per random tick (here 0.1, +0.02 per activator up to five, +0.02 for water,
   +0.02 for sky) and the activator list.
5. Feasts: servings (4), the leftover block at 0 servings, and that a bowl is the container for all four.
6. Food values (nutrition, saturation) and stack size 16 of meals; knife damage and durability per tier;
   block hardness. `data/farmersdelight/horizon/**/*.json` are placeholders until
   `horizon-port-pack carry` reads them (and the recipes, models, loot tables and tags) from the jar.
7. Whether the cutting board takes one item or the whole stack, and which hand the tool must be in.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
