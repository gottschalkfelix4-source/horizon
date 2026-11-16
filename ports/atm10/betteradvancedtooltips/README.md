# Better Advanced Tooltips (Horizon port)

Port of Better Advanced Tooltips 2101.1.0 (ATM10 8.x, MIT) as a native Horizon client mod.
Written from the mod's observed behaviour and public description, not from its code.

## What it does

With advanced tooltips on (F3+H), these lines are added in dark grey, just before vanilla's own
advanced lines (the item id and the component count):

| Feature | Status |
|---|---|
| Durability of an undamaged item (vanilla shows it only once damaged), vanilla's `item.durability` text | ported |
| Food values: hunger and saturation from `minecraft:food` | ported |
| Burn time as furnace fuel, in ticks | ported |
| Size of the item's saved data (`ItemStack` as NBT, Java binary size) | ported |
| The item's tags | needs API |
| Anything shown without F3+H | none, as the original |

## Needs from Horizon

- **An item's tags.** `Tags` (`crates/horizon-mod-vocab/src/world/tags.rs`) answers whether an
  item is in a tag and a tag's members, not which tags an item has. Proposed:
  `fn item_tags(&self, item: ItemKey) -> Option<Vec<Identifier>>` on `Tags` (the merged item tags
  the client has, sorted by id; None while tags are unavailable).

## To check against the NeoForge original

1. Its exact list of lines, their order, wording and colour, and whether some need Shift.
2. Whether "NBT size" there is the whole stack or only its components, and in which unit.
3. The author and upstream URL for LICENSE-ORIGINAL.md.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
