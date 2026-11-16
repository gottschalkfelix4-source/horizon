# Occultism (Horizon port)

Port of [Occultism](https://github.com/klikli-dev/occultism) 1.224.5 (ATM10 8.x, MIT) as a native
Horizon mod. Written from the mod's observed behaviour and public description (the Dictionary of
Spirits guide book, the wiki), not from its code. The recipes and the way spirit fire is made are
**from memory** and must be checked against the original.

## What it does

| Feature | Status |
|---|---|
| Spiritfire: a Demon's Dream fruit (`occultism:datura`) dropped into fire or soul fire is used up and the fire becomes `occultism:spirit_fire` | ported |
| Spiritfire transformations: an item entity of a recipe input in spirit fire turns into the result, whole stack, count kept (diamond → spirit attuned gem, andesite → otherstone) | partly: the result comes from the loaded `occultism:spirit_fire` recipes (`recipes_of_type`) when there are any, else from a fixed table of two; only the table's inputs are listened to |
| Spiritfire never burns items or entities, does not spread | partly: the placeholder template (`minecraft:structure_void`) does nothing; the carried block facts decide |
| Demon's Dream (datura) crop, Third Eye effect, otherworld blocks seen only with it | not yet |
| Chalk, pentacles and the Golden Sacrificial Bowl rituals (summoning foliot, djinni, afrit, marid) | not yet |
| Spirits as workers: miners in the Dimensional Mineshaft, crushers, lumberjacks, transporters, janitors | not yet |
| Storage: Dimensional Matrix, Storage Controller, stabilizers, storage remotes | not yet |
| Familiars (bat, beholder, deer, devil, ...) | not yet |
| Divination Rod, Soul Gems, Satchel, Infused Pickaxe, Iesnium, sacrificial mob rituals | not yet |

How: a part is attached to the fruit and to each recipe input. On the item entity's server tick
(`on_entity_item_update`) it reads the block at the item's position: fruit over fire sets the
block to spirit fire and takes one fruit off the stack; an input over spirit fire has its stack
replaced by the result with the same count, with a fizz sound. The result is read from the
loaded `occultism:spirit_fire` recipes (`ingredient`, `result`: the format is from memory) and
falls back to the fixed table when no data pack has any.

## To check against the NeoForge original

1. How spirit fire is made (this port: fruit thrown into fire or soul fire; one fruit used).
2. The full `occultism:spirit_fire` recipe list, and whether a recipe converts the whole stack
   at once or one item at a time, and after how many ticks.
3. Whether spirit fire burns out, needs a block below, and how it treats entities.
4. The blocks' and items' facts come from `horizon-port-pack carry` on the jar;
   `data/occultism/horizon/**.json` are placeholders for building without it.

## Needs from Horizon

- **Listening to items a data pack names**: parts can only be attached at registration, before
  recipes load, so an input that only a data pack's `occultism:spirit_fire` recipe names is not
  heard. Proposed: `Registrar::item_part_dynamic(&mut self, part: Arc<dyn ItemPart>, filter: fn(&dyn
  Host, ItemKey) -> bool)` in `crates/horizon-mod-vocab/src/parts/items.rs`, re-evaluated after
  each data reload.
- **A block part for "entity inside" that can write the entity**: `entity_inside` hands a
  `LevelRandomHost`, which cannot change an item entity's stack; a server variant
  `BlockPart::entity_inside_server(&self, host: &mut dyn ServerHost, event: &mut
  EntityInsideEvent<'_>)` would let the fire own the transformation instead of a part on every
  input item, and make it work for inputs a data pack adds.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
