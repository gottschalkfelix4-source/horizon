# Lootr (Horizon port)

Port of Lootr 1.21.1-1.11.38.127 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's
observed behaviour and public description, not from its code. Only a loot chest block of its own
is ported: Lootr's real effect, turning every vanilla loot chest, barrel and minecart in the world
into a per-player one, is not.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| `lootr:loot_chest` | Barrel-template block with a 27-slot container in the vanilla 9x3 menu. Each player has an inventory of their own in the chest's saved `players` compound | ported |
| First open | Rolls the chest's `LootTable` (a saved string; set with `/data`) for that player with `LevelRef::roll_loot`, and scatters the stacks over distinct slots by a seed of the player and position | ported |
| Later opens | Restores exactly what that player left | ported |
| Sharing | One viewer at a time: a second player is turned away with an action-bar message (the container is one set of slots that is swapped per player) | the port's own limit |
| Automation | Hoppers and pipes cannot reach the chest | ported |
| Vanilla chests, barrels, minecarts and structure loot replaced by Lootr's, decay and refresh timers, ownership and admin commands (`/lootr`), trapped and shulker variants, per-player breaking rules | | not yet |

## Needs from Horizon

1. Turning vanilla loot containers into per-player ones: a hook at the moment a loot container is first opened (before its table is rolled into the shared inventory) that can hand the player a private menu, or a way to keep per-player item lists in an attachment on a vanilla block entity (`Holder::Level`/block attachments exist, but the menu would still be the shared container's). `container_loot_filled` runs once for the shared fill.
2. Menus over a player-private list of items whose clicks move items: `open_menu` shows items "to look at" and its clicks move nothing, so the port swaps one shared container's slots on `player_container_open` / `player_container_close` and allows one viewer.
3. A block-placed-by-worldgen hook (a feature or structure piece replacing `minecraft:chest` with `lootr:loot_chest` keeping its `LootTable`).

## To check against the NeoForge original

1. Whether each player's inventory is rolled once and stays, and the decay/refresh rules and defaults.
2. Block ids and names (`lootr_chest`, `lootr_barrel`, ...), and what breaking a Lootr chest does.
3. Whether two players may have the chest open together in the original (yes, each sees their own).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
