# Just Enough Breeding (Horizon port)

Port of Just Enough Breeding (ATM10 8.x, MIT) as a native Horizon client mod and a plugin of the
[JEI port](../jei/). Written from the mod's observed behaviour and public description, not from
its code. The ATM10 version is to be read from the jar (`horizon-mod.toml` says 1.0.0 until then).

## What it does

| Feature | Status |
|---|---|
| JEI category "Breeding": one recipe per animal, the animal drawn (following the mouse), its breeding items, an arrow, and what breeding gives | ported, not yet checked against the original |
| Breeding items from vanilla's food tags as the client has them (`minecraft:cow_food`, ...), so data packs' changes show | ported |
| `U` on wheat lists the cow, the mooshroom and the sheep; `R` on a spawn egg (or a turtle egg, frogspawn, a sniffer egg) lists the animal it comes from | ported |
| Vanilla's 24 breedable animals (cow, mooshroom, sheep, pig, chicken, horse, donkey, llama, cat, ocelot, wolf, fox, rabbit, turtle, panda, bee, goat, strider, hoglin, frog, axolotl, camel, sniffer, armadillo) | ported |
| Other mods' animals | needs API (below) |
| Extra notes per animal (tamed only for wolves and cats, the baby's drops, egg laying conditions) | not yet |

## Needs from Horizon

- **What an entity type is bred with.** Breeding items live in each animal's Java `isFood`, which
  vanilla backs with the food tags above but mods need not. Proposed in
  `crates/horizon-mod-vocab/src/host/entities.rs`:
  `fn entity_type_breeding_items(&self, kind: EntityTypeKey) -> Option<Vec<ItemKey>>` (None for a
  type that doesn't breed), and `fn entity_type_spawn_egg(&self, kind: EntityTypeKey) -> Option<ItemKey>`
  for the offspring slot.

## To check against the original (on a machine with the jar)

1. The recipe's layout and size, the category's icon, and whether the output is the spawn egg.
2. Whether trader llamas, mules (not breedable), skeleton horses and modded animals appear.
3. Which animals' extra conditions it notes.

## Build

Needs the JEI port beside it (`../jei`).

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
