# Modular Routers (Horizon port)

Port of [Modular Routers](https://github.com/desht/ModularRouters) 13.2.7 (ATM10 8.x, MIT) as a
native Horizon mod. Written from the mod's observed behaviour and public description, not from its code.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| `modularrouters:modular_router` | Furnace template (`facing`); block entity `modularrouters:modular_router`; 18 slots in vanilla's 9x2 chest menu: slot 0 buffer, 1-9 modules, 10-14 upgrades; hoppers and pipes reach the buffer from every side | ported |
| Run cycle | Every `tick_rate` ticks the modules run in slot order on the buffer | ported |
| Speed upgrade | 20 ticks between runs, 2 fewer per upgrade, never below 2 | ported, to check |
| Stack upgrade | Items per operation 1, doubled per upgrade, at most 64 | ported, to check |
| Range upgrade | +1 block range per upgrade for ranged modules, up to the module's cap | ported, to check |
| Sender Mk1 | Pushes the buffer into the inventory beside the router in the module's direction | ported |
| Sender Mk2 | Pushes into the first inventory along the line, range 8 (max 16) | ported, to check |
| Puller Mk1 | Pulls the first extractable stack of the inventory beside it into the buffer | ported |
| Dropper | Drops items from the buffer as an item entity in front | ported |
| Placer | Places the buffer's block in front when the space is replaceable | ported (default state only, no facing) |
| Breaker | Breaks the block in front; drops go into the buffer, the rest falls in the world | ported (no tool, no silk touch / fortune augment) |
| Vacuum | Picks up item entities within range 6 (max 12) into the buffer | ported, to check |
| Void | Deletes items from the buffer | ported |
| Module direction | Sneak-use a module in the air to cycle front, back, up, down, left, right (saved in `minecraft:custom_data`) | port's stand-in for the module GUI |
| Module filters (item list, mod, tag, regex), whitelist/blacklist, match damage/components | | not yet |
| Augments (pickup delay, redstone, regulator, range, stack, filter round-robin...) | | not yet |
| Redstone modes (always, high, low, never, pulse), router ecomode, energy use | | not yet: always on |
| Sender Mk3, Puller Mk2, Extruders, Activator, Detector, Flinger, Player, Fluid, Distributor, Energy modules; Camouflage, Fluid, Muffler, Security, Sync, Energy upgrades; Template frame | | not yet |

## Needs from Horizon

- A mod menu with a client screen without writing a Horizon-only client part: the module
  configuration GUI (direction buttons, filter slots, augment slots) needs `open_mod_menu` plus a
  `screen` part; this port keeps settings on the item and cycles them on sneak-use instead.
- `menu_slot_rules` for a vanilla generic menu opened by a block container, so the router's module
  slots take only modules and its upgrade slots only upgrades (today any item goes anywhere; the
  tick ignores what doesn't belong).
- A way to place a block "as a player would" from an item stack with a facing (a
  `BlockItem.place` with a `BlockPlaceContext`) for the placer module; `set_block` with the
  default state places blocks without their facing or block entity data.

## To check against the NeoForge original

1. Base tick rate (20), ticks per speed upgrade (2), and the minimum (2).
2. Stack upgrade: whether it doubles (1, 2, 4 ... 64) or adds a fixed amount per upgrade.
3. Ranges: Sender Mk2 base 8 / max 16, Vacuum base 6 / max 12, +1 per range upgrade.
4. Which way "left" and "right" turn relative to the router's front.
5. Puller: one stack's worth or `items_per_op` from the first non-empty slot only.
6. Breaker: whether the broken block's drops that don't fit stay in the world or block the break.
7. Block facts (hardness, recipes, loot) are placeholders in `data/modularrouters/horizon/` until
   `horizon-port-pack carry` reads them from the jar.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
