# EnderStorage (Horizon port)

Port of EnderStorage 2.13.0.191 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's
observed behaviour and public description, not from its code.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| Ender Chest (`enderstorage:ender_chest`) | 27 slots shared by every chest of the same frequency, in every dimension; vanilla chest menu; hoppers work on the block entity's copy | ported |
| Frequency | Three lid buttons (dye colours, white by default); a dye click on a button sets its colour | ported |
| Personal storage | A diamond on the latch makes the storage belong to the clicking player; only they open it; shared only with their own chests of the same colours | ported |
| Ender Tank (`enderstorage:ender_tank`) | 16 buckets shared by frequency, filled and emptied with buckets and through the fluid capability | ported |
| Ender Pouch | Sneak-use on a chest copies its frequency; use opens that chest's contents from anywhere (clicks carried out by the mod) | ported |
| Shared storage persistence | Saved under `horizon-mod-data/enderstorage/` at the end of a changing tick | ported |
| Recipes, lid and button models, open/close animation and sounds | | not yet (client / assets) |
| Ender Tank's redstone output and auto-eject, Ender Pouch's tooltip | | not yet |

## Needs from Horizon

- A block entity container backed by a shared store (or `ModContainer` over a world-level attachment), so two
  blocks of one frequency edited in the same tick do not lose an edit: today each block keeps a copy that is
  reconciled every tick, and the later one loses its edit.
- A menu opened over a mod-supplied container for items (the pouch) with the game moving the stacks, instead of the
  mod carrying out each `menu_clicked` itself.

## To check against the NeoForge original

1. Chest size (27 slots, the default `item storage size` of 1) and tank capacity (16 buckets).
2. Where on the lid each of the three buttons sits and where the latch is (`spot` in `src/lib.rs`).
3. Whether a dye is used up in creative mode, and whether a personal storage may be recoloured by others.
4. Pouch behaviour: sneak-use to bind, and whether it opens personal storages of other players.
5. Hardness and blast resistance (`data/enderstorage/horizon/block/*.json` are placeholders).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
