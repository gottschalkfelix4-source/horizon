# Applied Energistics 2 (Horizon port)

Port of [Applied Energistics 2](https://github.com/AppliedEnergistics/Applied-Energistics-2) 19.2 (ATM10 8.x,
LGPL-3.0) as a native Horizon mod, in stages. Written from the mod's observed behaviour, not from its
code. Stage 1 is the network and its power; the later stages sit on the network model in `src/rules.rs`.

## Stages

| Stage | Content | Status |
|---|---|---|
| 1 | Cables (glass, covered, smart, dense covered, dense smart; fluix and 16 colours), controller, energy acceptor, energy cell, dense energy cell, network scan and power model; crystals, dusts, processors as data | ported, numbers to check; not run in the game |
| 2 | Drive (ten cell slots), item storage cells 1k to 256k with contents saved in the stack, `network_insert` / `network_extract` over a network's cells, storage summary on the leader | ported, not run in the game. Not yet: ME chest, fluid cells, cell workbench and partitioning, inscriber and presses, charger, certus growth |
| 3 | ME terminal with search, crafting terminal, pattern terminal | not yet |
| 4 | Import/export buses, storage bus, interface, annihilation and formation planes, p2p tunnels | not yet |
| 5 | Autocrafting: patterns, pattern provider, molecular assembler, crafting CPU multiblock | not yet |
| 6 | Spatial IO, wireless access points and terminals, quantum bridge, meteorites, world generation | not yet |

## Stage 1 behaviour

- A network is every AE block joined face to face. Devices join any neighbour; cables join unless both
  have a colour and the colours differ (fluix joins every colour).
- The lowest block of a network runs it each tick (scan stops as soon as a lower block is met, cap 8192
  blocks). Energy acceptors hand their FE to the cells; the idle draw is taken from the cells; the leader saves
  `nodes`, `powered`, `stored` and the carried hundredths of FE in its block entity data.
- Power: 1 AE = 2 FE. Cell 200 000 AE, dense cell 1 600 000 AE, acceptor buffer 800 FE.
  Idle draw: controller 6 AE/tick, smart cable 0.1, dense smart cable 0.4 (glass and covered cables 0).
- A network is powered when its cells hold the whole draw of the tick.

## Stage 2 behaviour

- A cell's contents are `{<item id>: <count>}` under `ae2_cell` in the stack's `minecraft:custom_data`. A 1k cell holds
  8128 of one type: 1024 bytes, 8 per type (bytes/128), 8 items per byte, 63 types at most.
- The drive keeps 18 slots (vanilla's 9x2 menu until a mod menu exists); only the first ten count.
- The leader saves `types`, `items`, `bytes_used` and `bytes_total` of all cells in the network's drives.
  Terminal and buses (stages 3 and 4) call `network_insert`/`network_extract`, each one transaction.
- Not checked: the cell slot filter (any item can be put in the drive; non-cells are ignored).

## Not in stage 1

Channels (8 per cable, 32 dense; nothing needs one yet), the vibration chamber, creative cell, P2P, facades and cable
parts, the cell and item textures, recipes and world generation (`horizon-port-pack carry` brings the data).

## Needs from Horizon

Collected in `../ENGINE-GAPS.md` under "ae2":

1. Per-face "connectable" queries without ticking: the network is scanned every tick by its lowest block (cheap
   for small networks, wasteful for large ones). A neighbour-changed hook per block (`neighbor_changed`) plus a
   mod-held graph cache would replace the scan.
2. Reading another block's saved data (`BlockEntityRef` of a block entity at a position) so every node can show
   the network's `powered` state; today only the leader knows.
3. Energy capability faces that differ for insert and extract by a rule (the acceptor must not give energy out to
   outside cables; the port allows it).

## To check against the NeoForge original

1. Every number above (capacities, idle draw, FE/AE ratio, whether cells fill from outside at once).
2. Cable colour joining and the dense-cable rules.
3. Hardness and resistance (placeholders until `carry`).

## Build

```sh
sh ../dev-sdk.sh        # once: links the checkout's mod API
cargo test
python3 gen_data.py     # rewrites data/
horizon-launcher native-build . --instance <instance> --trust-native-code
```

## Fixes after test run 12

- A network is led by a cable or the controller (a block's own energy and items cannot be read during its
  own tick, which left cells and acceptors at 0 when one of them was the lowest block); only a network
  with no cable or controller falls back to a device as leader.
- Powered needs stored energy above 0 (an empty network is no longer powered).
- Blocks drop themselves (`data/ae2/loot_table/blocks/`, written once, not by gen_data.py); certus quartz
  ore drops crystals with fortune (`ore_drops`), itself with silk touch.
- The drive takes only storage cells, in slots 0 to 9 (`menu_slot_rules` on `minecraft:generic_9x2`).
- Storage cells stack to 1 (two cells used to merge into one stack of 2 that counted once).
- `item replace` can still put any item into a drive slot; the network ignores non-cells and slots past ten.
