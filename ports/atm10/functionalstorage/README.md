# Functional Storage (Horizon port)

Port of Functional Storage 1.21.1-1.5.8 (ATM10 8.x, MIT) as a native Horizon mod. Written from the
mod's observed behaviour and public description, not from its code. Only the plain drawers are
ported.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| `<wood>_1`, `_2`, `_4` drawers (six woods) | One block entity type `functionalstorage:drawer`; 1, 2 or 4 slots, each one kind of item, holding 32, 16 or 8 stacks | ported, numbers assumed |
| Automation | An `ITEMS` provider (a custom `ResourceHandler`), so hoppers, pipes and routers insert into and extract from every slot; a kind never takes two slots of a drawer | ported |
| Hand use | Using with a stack puts the held stack in (all that fits); using with an empty hand takes a stack from the last filled slot, one item when sneaking | ported (the original uses left and right click on the slot hit) |
| Contents saved as item nbt and count | | ported |
| Compacting and simple compacting drawers, storage controller, armory cabinet, ender drawer, fluid drawer, upgrades (storage multipliers, void, redstone, pulling/pushing), locking, framed drawers, item count display | | not yet |

## Needs from Horizon

1. A left-click (attack) hook on blocks (`BlockPart::attack_block` with the hit position) so a drawer can give items on punch and choose its slot by where it was hit; today a use with an empty hand is the only way.
2. A client renderer for item counts and the item on a drawer face (a block entity renderer with the item and a count text); until then a drawer looks like its planks.
3. Mod menu screens for upgrade slots and the controller.

## To check against the NeoForge original

1. Capacity per slot: 32 stacks for the 1x1, 16 for the 1x2 and 8 for the 1x4 drawer.
2. The block ids (`oak_1`, `oak_2`, `oak_4`) and the wood list.
3. Whether a kind may sit in two slots of one drawer (assumed not).
4. The hand rules: right-click inserts the held stack, a double click inserts the whole inventory's matches (not ported).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
