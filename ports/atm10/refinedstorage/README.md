# Refined Storage (Horizon port)

Port of [Refined Storage](https://github.com/refinedmods/refinedstorage2) 2.0.9 (ATM10 8.x, MIT) as a
native Horizon mod. Written from the mod's observed behaviour and public description, not from
its code. The first slice is the **storage network** itself.

## What it does

A `refinedstorage:controller` joins every Storage Block it reaches through `refinedstorage:cable`
(and through the storage blocks themselves: every network block carries the connection) into one
inventory. The 1k, 4k, 16k and 64k Storage Blocks hold 1 000 to 64 000 **items** of any mix (no
type limit, no stack sizes), saved in their block entity under `stored`. The Controller answers
`horizon:items` with the whole network, so a hopper, pipe or importer on any side of it fills or
empties the network: an item goes first to storages already holding it, then to any with room.
A second Controller reached by the same network makes it a conflict, and the network answers
nothing. Items whose mod is gone are kept and counted, not deleted.

| Feature | Status |
|---|---|
| Controller + Cables + Storage Blocks as one inventory, through `horizon:items` | ported, not yet checked against the original |
| Two controllers conflict | ported |
| Controller energy (FE buffer, use per device, network off without energy) | not yet |
| Grid / Crafting Grid / Pattern Grid screens | not yet |
| Disk Drive and Storage Disks (items and fluids), Storage Housing | not yet |
| Keeping a Storage Block's contents on its item when broken | not yet: breaking one loses what it held |
| Importer, Exporter, External Storage, Interface, Constructor, Destructor, Detector | not yet |
| Autocrafting (patterns, Crafter, Crafting Monitor) | not yet |
| Wireless Grid, Network Transmitter, upgrades, priorities, filters, fluid storage | not yet |

## To check against the NeoForge original (oracle on a machine with the jar)

All of these are from memory:

1. Block ids (`refinedstorage:controller`, `refinedstorage:cable`,
   `refinedstorage:{1k,4k,16k,64k}_storage_block`) and the capacities.
2. Insert order across storages: the original orders by storage priority, then by what? Here:
   storages already holding the item first, then network order (breadth-first from the
   controller).
3. Whether storage blocks carry the network on to blocks behind them (here they do).
4. The blocks' facts: `data/refinedstorage/horizon/` holds placeholders until
   `horizon-port-pack carry` brings the jar's own.

## Needs from Horizon

- None blocking this slice. Each capability call walks the network (bounded at 4 096 blocks);
  a cached graph, invalidated from the existing `block_changed` hook, is a later optimisation.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
