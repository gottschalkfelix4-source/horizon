# Enchantment Descriptions (Horizon port)

Port of [Enchantment Descriptions](https://github.com/Darkhax-Minecraft/Enchantment-Descriptions)
21.1.11 (ATM10 8.x, LGPL-2.1-only) as a native Horizon client mod, released under the same
LGPL-2.1-only licence. Written from the mod's observed behaviour and public description, not from
its code; the description texts in `assets/enchdesc/lang/en_us.json` are this port's own.

## What it does

| Feature | Status |
|---|---|
| A grey line under each enchantment of an item or enchanted book, from `enchantment.<ns>.<name>.desc` | ported |
| Descriptions for all 43 vanilla 26.3 enchantments (English) | ported (own wording) |
| Other mods' enchantments described by their own `...desc` keys (the same key scheme, so packs and mods that ship them work) | ported |
| Only while Shift is held (default on), with a "Hold Left Shift" hint otherwise | ported |
| Only on enchanted books (setting, default off) | ported |
| Descriptions in the enchanting table's hover tooltip | not yet: needs the table's clue tooltip in `item_tooltip` or a hook of its own |
| Other languages | not yet: only en_us is written |
| Configurable key instead of Shift, indentation | not yet |

How: `item_tooltip` reads `minecraft:enchantments` and `minecraft:stored_enchantments` (patch,
else the item's default), finds each enchantment's name line by its translation key and inserts
the description after it when the language has one (`ClientHost::translate`).

## Needs from Horizon

- **Enchanting table clues.** The table's hover tooltip (`EnchantmentScreen`, "Enchantment
  level?") is not an item tooltip. Proposed: a client hook `enchantment_clue_tooltip` in
  `crates/horizon-mod-vocab/src/hooks/enchantment_clue_tooltip.rs` with
  `EnchantmentClueTooltipEvent { slot: u8, enchantment: &str, level: u32, lines: &mut Vec<(Text, u32)> }`
  (contract `listen`, NeoForge counterpart: none, a mixin in the original).

## To check against the NeoForge original

1. The default of "Shift required", the hint's wording and which key it names.
2. Where the description goes for an enchantment shown twice (book and item) and for hidden
   enchantments (`tooltip_display`).
3. Colour and indentation of the description line.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
