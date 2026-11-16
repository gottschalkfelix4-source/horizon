# Industrial Foregoing (Horizon port)

Port of [Industrial Foregoing](https://github.com/InnovativeOnlineIndustries/Industrial-Foregoing)
1.21-3.6.39 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's observed behaviour and
public description, not from its code.

## What it does

Every machine is one block entity type, `industrialforegoing:machine`. Energy (FE) and tanks are
the game's own buffers (`r.block_energy`, `r.block_fluids`), so pipes, cables and hoppers reach
them through `ENERGY`, `FLUIDS` and `ITEMS`. A machine works on the block or area **behind** it
(the side its front faces away from). Its cycle fills by 1 progress a tick (more with speed
addons) while it has work and the energy for one operation; a full bar runs the operation.

| Part | Behaviour | Status |
|---|---|---|
| Fluid Extractor | Draws 3 mB latex per operation from the log behind it (5-tick cycle, 10 FE); 1% chance per operation that the log is used up (turns to air) | ported, to check |
| Latex Processing Unit | 750 mB latex + 500 mB water → 1 Tiny Dry Rubber (60 ticks, 40 FE); 9 tiny → Dry Rubber (crafting), Dry Rubber → Plastic (smelting) | ported, to check |
| Dissolution Chamber | 8 input slots + input tank → item and/or fluid, time per recipe | engine ported; 3 recipes carried (machine frame, pink slime ingot, dry rubber), the rest need the gap below |
| Mob Crusher | Kills one living mob per operation in a 3×3×3 area (60 ticks, 200 FE) through a fake player, so player-kill loot drops; picks up the item drops in the area into its outputs; never players, armor stands, the Wither or the Ender Dragon | ported; essence from experience not yet |
| Plant Gatherer | Harvests ripe crops (`minecraft:crops`, nether wart: replanted at age 0, one seed less), melons and pumpkins, sugar cane / cactus / bamboo above their base, in a 3×3 area; 10 mB sludge per harvest | ported; trees (logs and leaves) not yet |
| Plant Sower | Plants seeds and saplings from its inputs in a 3×3 area: crops on farmland, nether wart on soul sand, saplings on `minecraft:dirt` | ported; filter/fertiliser slots not yet |
| Pitiful Generator | Burns furnace fuel for its vanilla burn time, 30 FE/t; pushes up to 1000 FE/t into neighbours | ported |
| Biofuel Generator | Burns 4 mB biofuel a tick for 160 FE/t; pushes up to 2000 FE/t | ported (biofuel comes from the Bioreactor, not yet) |
| Black Hole Unit (pity, simple, advanced, supreme) | One item kind, 32 768 / 524 288 / 4 194 304 / 2³¹−1 items; slots 0–3 take input, slot 4 is the output kept full from the store; the store survives in the block entity | ported; keeping contents when broken not yet |
| Black Hole Tank (four tiers) | One fluid, 64 000 / 1 024 000 / 16 384 000 / 2³¹−1 mB, through `FLUIDS` | ported; bucket click and contents on break not yet |
| Addons: speed, efficiency, processing (tiers 1–2), range (`range_addon0`–`11`) | Items whose `minecraft:custom_data` carries `industrialforegoing:addon {kind, tier}`; in a machine's addon slots (those neither input nor output): +1 progress/tick per speed tier, −10% energy per efficiency tier (floor 10%), +1 operation per cycle per processing tier, +1 area radius per range tier. The best addon of each kind counts. | ported, to check |
| Bioreactor, Sludge Refiner, Laser Drills, Animal machines, Enchantment machines, conveyors, Infinity tools, side configuration, redstone control, machine GUIs | | not yet |

Machines open vanilla menus (`generic_9x1`, `generic_9x2`, `hopper`, `furnace`) in place of IF's
own screens, which a server mod cannot add; the extra slots of those menus are the addon slots.

Fluids `industrialforegoing:{latex, sludge, biofuel, pink_slime, essence}` are tank-only
(`data/industrialforegoing/horizon/fluid/`).

## Needs from Horizon

- **Mod recipe types' own fields.** `Host::recipes_of_type("industrialforegoing:dissolution_chamber")`
  gives a recipe's ingredients and results but not its `input_fluid`, `output_fluid` or
  `processing_time`, so the chamber carries a few recipes in code instead of the mod's data
  pack. Wanted: the recipe's JSON (`Recipe::json: Option<String>`), or
  `Host::recipe_json(id) -> Option<String>`.
- **Experience of a kill.** The Mob Crusher turns a kill's experience into essence; there is no
  read of an experience orb's value (`ExperienceOrbRef::value`) or of the experience a death
  awards (`LivingRef::experience_reward`).
- **Mod menus with a server-only mod.** A `menu/` type needs the client's own screen; a
  server-side port has none, so machines use vanilla menus and cannot show energy, tanks or
  progress bars.
- **Block entity contents on the dropped item** (Black Hole Unit/Tank keep their store when
  broken): a `block_entity_item_drops`-style hook that writes `minecraft:block_entity_data` into the
  drop, and reads it back on placement.

## To check against the NeoForge original

1. Every number in `src/rules.rs`: cycle lengths, FE per operation, buffer sizes, tank sizes
   (8000 mB), latex per operation (3 mB) and the log's break chance (1%), what the log becomes
   (air here), the Latex Processing Unit's 750/500 mB, sludge per harvest, generator outputs.
2. Which side the machines work on (behind here), the area's size and height per machine, and
   how range addons grow it.
3. Addon effects and their tiers; whether two addons of one kind stack. The `custom_data`
   contract is the port's own (IF uses an addon item class).
4. Black Hole tier capacities; whether the Black Hole Unit output slot is kept full.
5. Whether the Mob Crusher spares babies and named mobs by default, and the order it picks mobs.
6. The Plant Gatherer's replant rule (one seed kept back) and what it does with stalks.
7. Block facts (`data/industrialforegoing/horizon/block/*.json` are placeholders until
   `horizon-port-pack carry` reads them, and the recipes, models and loot tables, from the jar).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
