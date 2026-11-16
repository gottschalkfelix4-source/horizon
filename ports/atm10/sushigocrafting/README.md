# Sushi Go Crafting (Horizon port)

Port of Sushi Go Crafting 0.6.6 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's
public listing, not from its code. **I could not recall the original's items and crafting.** This
port assumes sushi made from rice, nori and fillings on a rolling mat, and gives the roll a food
value from its fillings; none of the original's ids, crops or recipes are reproduced.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| `rolling_mat` | Wool-template block with five slots in vanilla's hopper menu: rice (0), nori (1), two filling slots (2, 3), result (4); rolls for 40 ticks and consumes one of each | assumed |
| `sushi_roll` | Its `minecraft:food` component is worked out per roll: 3 nutrition plus 4 salmon, 3 cod, 2 tropical fish, 1 carrot, dried kelp, wasabi or soy sauce per filling, at most 20, saturation 0.6 per point; rolls with the same value stack | assumed |
| Items `rice`, `nori`, `wasabi`, `soy_sauce` | Placeholder items (wheat-seed template) | assumed |
| Rice and wasabi crops, recipes to get rice, nori, wasabi and soy sauce, the original's roll types (maki, nigiri, ...), sauces and their effects, the original's block list | | not yet: the ingredients cannot be obtained in survival |

## Needs from Horizon

1. Data-driven machine/recipe types for custom crafting stations (see `crafting-recipe-kinds.md`): a rolling recipe with variable ingredients would be a recipe kind rather than a hard-coded table, but `recipe_for` only matches the grid kinds.
2. Mod menu screens (the mat opens the hopper menu, whose slots have no labels).

## To check against the NeoForge original

1. Everything above: what the mod adds, how sushi is crafted (a table, a mat, a crafting grid), and what the food values are.
2. Whether the original's items have components (ingredients stored on the roll) or are fixed items.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
