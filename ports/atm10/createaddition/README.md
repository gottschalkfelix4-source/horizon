# Create Crafts & Additions (Horizon port)

Port of Create Crafts & Additions 1.7.2 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's
data files (recipes, loot tables, tags, translations; `gen_data.py` upgrades them to 26.3's formats with
`port_convert.py`, ingredients as `"mod:item"` and `"#tag"`) and its observed behaviour, never from its
code. Built and unit-tested only; not run in the game.

It depends on the [Create port](../create/) only for ids: recipes name Create items the port declares
(andesite alloy, shaft, andesite casing, brass ingot, sheets...). Create's rotation network lives inside
Create's crate and cannot be joined from here, so the rotation blocks are an own energy part (below).

| Part | Behaviour | Status |
|---|---|---|
| Electric motor | Takes FE (8 FE per tick per RPM) and runs at its speed (8 to 256 RPM, an empty hand steps it, default 32); writes `speed_x1000` into its saved data | energy side ported; no rotation reaches Create |
| Alternator | Turned at some speed it makes FE (8 per RPM per tick, 90 %) and pushes it to neighbours; a running motor beside it turns it | stand-in coupling until Create can feed it |
| Modular accumulator | 2,000,000 FE buffer, 5,000 per tick in and out, gives to non-accumulators | one block, not the original's expandable multiblock |
| Connector, small light connector, large connector | Buffers (4,000 / 16,000 FE) that push to every neighbour (1,000 / 8,000 per tick) | no wire links between far connectors |
| Creative generator | Pushes 1,000,000 FE per tick to neighbours | ported |
| Tesla coil | Eight input slots and an output slot (3x3 menu, hoppers work), 10,000 FE buffer: charges the first item that has a `createaddition:charging` recipe at that recipe's rate until its energy is full (recipes are the original's: copper deoxidising, gold electrifying, channeling and others, 1.21.1 enchantment components converted) | ported; the original charges items near it, here they sit in its slots |
| Electrum, rods, wires, spools, capacitor, biomass, straw, goblets, amulet, cake blocks and others (31 items, 19 blocks) | Declared from the original's names with vanilla templates for look | data only |
| Recipes | 65 of the original's: crafting (shaped and shapeless), smoking, charging, rolling (`createaddition:rolling` is registered and readable, no machine runs it yet) | carried |

## Not ported

1. Create's own recipe types (mixing, compacting, pressing, filling, crushing, mechanical crafting: the
   motor, alternator, pump and coil recipes are mechanical crafting, so those four blocks have no recipe)
   and the Rolling Mill machine: Create's port reads its recipes in code and has no hook for another mod.
2. Recipes that need Create items the port lacks (brass casing, electron tube, mechanical bearing, chute),
   and every `compat` recipe (Mekanism, AE2, Immersive Engineering, Tinkers, Thermal).
3. Fluids (seed oil, bioethanol), the liquid blaze burner and `liquid_burning` recipes, the Servo motor,
   electric pump, portable energy interface, redstone relay, digital adapter and barbed wire behaviour
   (blocks are declared), potato projectiles, damage types.

## To check against the NeoForge original

Every machine number in `src/rules.rs` is from memory of the default config (capacities, transfer rates,
8 FE per RPM, 90 % alternator, speeds). The charging numbers are the original's data.

## Build

```sh
sh ../dev-sdk.sh && cargo test --lib
python3 gen_data.py <folder with the unpacked data of the original's jar>
```
