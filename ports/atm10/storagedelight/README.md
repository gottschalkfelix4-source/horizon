# Storage Delight (Horizon port)

Port of Storage Delight 26.09.17-1.21-neoforge (ATM10 8.x, MIT, an integration with Farmer's
Delight) as a native Horizon mod. Written from the mod's public listing, not from its code.
**I could not recall its blocks.** This port assumes food storage furniture and ships two blocks;
the real mod's block list, ids and recipes are not reproduced.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| `pantry` | Planks-template block, 27 slots in vanilla's 9x3 menu; a player's click puts only food (items with the `minecraft:food` default component) in | assumed |
| `fridge` | Iron-template block, 18 slots in the 9x2 menu; food, milk buckets, potions and honey bottles | assumed |
| Hoppers and pipes | Reach every slot | ported, but not filtered (see Needs) |
| The original's other storage blocks (baskets, crates, cabinets, shelves per wood), Farmer's Delight pairing, freshness or cooling | | not yet |

## Needs from Horizon

1. Per-slot validity for `ModContainer` so hoppers and pipes obey the food filter (today only `menu_slot_rules`, which is per menu type and clicks only, can filter).
2. Mod menu screens and block entity renderers (the contents are not shown on the block).

## To check against the NeoForge original

1. Which blocks the mod adds, their ids, slot counts and what they accept.
2. Whether any of them keeps food fresh or interacts with Farmer's Delight's cooking pot or stove.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
