# Theurgy (Horizon port)

Port of [Theurgy](https://github.com/klikli-dev/theurgy) 1.78.0 (ATM10 8.x, MIT) as a native
Horizon mod. Written from the mod's observed behaviour and public description (its guide book,
the wiki), not from its code. Ranges, uses and the way a rod is attuned are **from memory** and
must be checked against the original.

## What it does

| Feature | Status |
|---|---|
| Divination Rods T1-T4: sneak + right click on a block attunes the rod (block id kept in the stack's custom data) | ported |
| Divination Rods: right click searches a cube around the player (radius 16 / 32 / 48 / 64 by tier), nearest first, writes the find to the stack and tells distance and compass direction on the action bar; one durability per search (T1 5, T2 10, T3 25 uses, T4 unbreakable) | ported |
| Divination Rods: hold-to-use timing, the rod's model pointing at the find, particles | not yet (client side) |
| Which blocks each tier may attune to (block tags per tier), attuning by crafting with an ingredient | not yet |
| Alchemical sulfur, salt, mercury, niter; reformation and fermentation | not yet |
| Calcination Oven, Liquefaction Cauldron, Distiller, Incubator, Mercury Catalyst, Caloricum generators | not yet |
| Reformation Array, Fermentation Vat, Digestion Vat, Sal Ammoniac Accumulator and Tank | not yet |
| Mercury flux network (Mercury Shards, Mercury Catalyst feeding machines) | not yet |
| Guide book (Modonomicon) | not yet |

How: a part on each rod's item. `use_on` (server side) attunes while the player's secondary use
(sneak) is active; `use_item` searches with one block read per position, shell by shell from
the player outwards, and stops once no farther shell can hold a nearer find. The stack is
changed through the player's hand (`ItemPlace::PlayerHand`, in a transaction); a worn-out rod
breaks. The messages are translation keys (`assets/theurgy/lang/en_us.json`).

## To check against the NeoForge original

1. Search radius and vertical range per tier, number of uses per tier, and whether a search
   that finds nothing still costs a use.
2. How a rod is attuned (sneak-click; crafting with the target block or a sulfur), which blocks
   each tier accepts, and the data component names the rod uses.
3. The search's notion of "nearest" (straight-line here) and what the original reports (it
   shows direction through the rod's model and particles; this port uses the action bar).
4. The items' facts come from `horizon-port-pack carry` on the jar;
   `data/theurgy/horizon/item/*.json` are placeholders for building without it.

## Needs from Horizon

- **A bulk block read of a box**: the search makes one host call per position (274,625 for the
  T2 cube). `LevelRef::blocks_in_box(self, host: &dyn Host, min: [i32; 3], max: [i32; 3]) ->
  Option<Vec<BlockKey>>` (x, then z, then y order; `None` past a size cap or where unloaded) in
  `crates/horizon-mod-vocab/src/host/levels.rs` would make it one call per chunk section.
  `read_columns` only reads around a heightmap.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
