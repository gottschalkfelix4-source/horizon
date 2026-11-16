# Bibliocraft Legacy (Horizon port)

Port of Bibliocraft Legacy 1.6.5 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's
observed behaviour and public description, not from its code. Bibliocraft is a large furniture and
decoration mod; only two of its storage blocks are ported.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| `<wood>_bookcase` (six overworld woods) | 18-slot container in vanilla's 9x2 menu that takes only books (book, writable, written, enchanted, knowledge) when a player clicks | ported (slot count assumed) |
| `<wood>_potion_shelf` | 9-slot container in the 9x1 menu that takes only bottles (potions, splash, lingering, glass bottle, experience and honey bottles) | ported (assumed) |
| Hoppers and pipes | Reach every slot | ported, but they are not limited to books or bottles (see Needs) |
| Shelf, label, desk, tool rack, armor stand, framed wall/frame, painting frames, clipboards, fancy workbench, printing press, typesetting machine, slotted book, furniture (tables, chairs, lamps), all tools and items | | not yet |
| Models that show the books or bottles on the shelf | | not yet (client) |

## Needs from Horizon

1. Per-slot validity for `ModContainer` (a filter on what the container's `ITEMS` handler accepts), so hoppers and pipes obey the same rule as clicks; today only `menu_slot_rules` (per menu type, clicks only) can filter, which also applies to every other container using the 9x2 and 9x1 menus (the rule checks the block at the menu's position, so other blocks are untouched).
2. Block entity renderers that draw the contents on the shelf.
3. Mod menu screens (bookcase and potion shelf use vanilla's generic menus).

## To check against the NeoForge original

1. Slot counts: the original bookcase has 16 slots; and what each shelf accepts.
2. The set of blocks and woods, and block ids.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
