# Apothic Enchanting (Horizon port)

Port of Apothic Enchanting 1.6.4 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's
observed behaviour and public description, not from its code. Only four enchanting shelves and a
book library are ported; the part that makes the mod what it is, enchanting far beyond level 30,
is not possible with the engine's table today.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| `hellshelf`, `infused_hellshelf`, `seashelf`, `infused_seashelf` | Add to an enchanting table's power through `enchant_power_bonus`. The port sums the eterna of every mod shelf in vanilla's ring around the table (1.5 or 2.0 each) and holds it to the smallest maximum of any kind present (22.5 or 45); each shelf cell reports its share | ported, numbers assumed |
| `library` | 18-slot container in vanilla's 9x2 menu that takes enchanted books only when a player clicks | ported (assumed) |
| Quanta, arcana and rectification, the table's eterna/quanta/arcana screen, the tome and its recipes, treasure enchantments, all other shelves (beeshelf, melonshelf, sightshelf, deepshelf, echoing, ...), library extraction rules, enchantment level beyond vanilla | | not yet |

## Needs from Horizon

1. **Uncapped table power.** The engine's `enchant_power_bonus` caps a table's power at 15 for the offers (vanilla's rule), so eterna above 15 has no effect: level 30 is the most a table offers. Apothic needs the cap and the level formula (`power * 2`, up to 100) to be changeable (a hook on the offers' computation, or a `max_power` field on the event).
2. A hook to add statistics (quanta: randomness; arcana: rarity of treasure) and to pick the enchantments for an offer (`prepare_item_enchant` exists but cannot see the stats).
3. Per-slot validity on `ModContainer`: hoppers and pipes can put anything in the library; only a player's click is filtered (`menu_slot_rules` applies per menu type and the rule checks the block at the menu's position).
4. Mod menu screens for the table's stat bars.

## To check against the NeoForge original

1. Each shelf's eterna, maximum and the shelf list; what quanta and arcana each adds.
2. Whether the ring and its air-gap rule are vanilla's (the port ignores the gap rule, counting any shelf in the ring).
3. The library's slot count and the rules for taking books out.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
