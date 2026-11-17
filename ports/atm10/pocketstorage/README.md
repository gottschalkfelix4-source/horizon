# Pocket Storage (Horizon port)

Port of Pocket Storage 1.2.5 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's public
listing, not from its code. **I could not recall what the original adds.** From its name this port
assumes pocket-sized storage and ships three storage blocks; if the original is a portable item
(a backpack-like pocket), that part is missing and is listed below.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| `pocket_pouch`, `pocket_chest`, `pocket_vault` | Chest-template blocks with 9, 27 and 54 slots in vanilla's generic menus; hoppers and pipes reach every slot; contents spill when broken | assumed |
| Sorting | Sneak-use with an empty hand merges stacks of one kind and orders them by item id | the port's own addition |
| A portable pocket item that opens a container and keeps its contents in the stack, upgrades, locking | | not yet |

## Needs from Horizon

1. For a portable storage item: a menu over an item's container whose slots write back into the stack. `open_menu` shows a plain list, and a mod learns of a close (`menu_closed`) but cannot be sure the menu can still be read then, and `menu_clicked` fires only for clicks that moved nothing. A `r.item_container` item with an `ItemPlace`-backed menu (slots bound to the stack, as a shulker box or bundle screen) would do it.
2. Mod menu screens (vanilla's generic menus are used).

## To check against the NeoForge original

1. What the mod is: blocks, an item, or both; what sizes it has.
2. Whether it has upgrades, filters or a sorting button.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
