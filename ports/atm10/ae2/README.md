# Applied Energistics 2 (Horizon port)

Port of [Applied Energistics 2](https://github.com/AppliedEnergistics/Applied-Energistics-2) 19.2 (ATM10 8.x,
LGPL-3.0) as a native Horizon mod, in stages. Written from the mod's observed behaviour, not from its
code. Stage 1 is the network and its power; the later stages sit on the network model in `src/rules.rs`.

## Stages

| Stage | Content | Status |
|---|---|---|
| 1 | Cables (glass, covered, smart, dense covered, dense smart; fluix and 16 colours), controller, energy acceptor, energy cell, dense energy cell, network scan and power model; crystals, dusts, processors as data | ported, numbers to check; not run in the game |
| 2 | Drive (ten cell slots), item storage cells 1k to 256k with contents saved in the stack, `network_insert` / `network_extract` over a network's cells, storage summary on the leader | ported, not run in the game. Not yet: ME chest, fluid cells, cell workbench and partitioning, inscriber and presses, charger, certus growth |
| 3 | ME terminal (six-row view: 45 listed items by count, paging, take out and put in, search by item on the cursor); channels | ported, not run in the game. Text search (client typing sent to the server as `ae2:search`). Not yet: crafting and pattern terminal |
| 4 | Import and export bus, channels, storage bus, interface | ported, not run in the game. Not yet: annihilation and formation planes, P2P, upgrade cards, priorities |
| 5 | Autocrafting: patterns, pattern provider, molecular assembler, recursive plans | ported, not run in the game. Stand-ins: no crafting CPU, no timing, the assembler is only the capacity a request needs; no pattern terminal, no processing patterns |
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
- Block tags `mineable/pickaxe` and `needs_stone_tool` (ores) are written by gen_data.py.
- Loot tables use 26.3's keys (`type` for conditions and functions, not 1.21.1's `condition`/`function`); the
  names of blocks and items are in `assets/ae2/lang/en_us.json`.

## Stage 3 behaviour (ME terminal)

- Right-click the terminal (it must be on a powered network, i.e. cells holding energy): a six-row view lists the
  network's items, most first. Left click takes a stack, right click half; with a stack on the cursor a click puts it
  in (right click one); shift-click on a listed item sends a stack to the inventory, on an inventory slot puts the
  stack into the network. Only plain stacks (no components) go in. Arrow slots turn the page; the compass slot sets the
  search to the item on the cursor (empty cursor clears it). A listed stack shows at most one stack's count.
- Idle draw 0.5 AE/tick. Text search and AE2's counts in k/M need client hooks (`screen_char_typed_pre`, `screen_draw`);
  stage 3b.

## Stage 4 behaviour (buses, channels)

- `ae2:import_bus` and `ae2:export_bus` are blocks joined to the network. Every 5 ticks a bus moves one item between the
  network's cells and the first neighbouring inventory (a block outside the network with an item capability) that can
  take or give it. Needs power and a channel, draws 0.1 AE/tick.
- The import bus takes the first plain item it finds (only its filter's item when it has one). The export bus puts out
  its filter's item, nothing without a filter. Right-click the bus to open it and put the item to filter by in its first slot (slots 1 to 4 take `ae2:speed_card`s, each card doubles the items per operation: 1, 2, 4, 8, 16; the other slots refuse items). AE2 buses are cable parts with a configuration screen; this is the port's stand-in. A filter saved by
  `data merge` (`filter`) also works.
- Channels: a network has 8, or 32 with a controller. Drives, terminals and buses each take one, the lowest positions
  first; a device past the limit does nothing (a drive's cells are not seen, a terminal says "no free channel"). AE2's
  per-cable channel routing (8 per smart cable, 32 per dense) is not modelled.

## Stage 5 behaviour (autocrafting)

- `ae2:blank_pattern`: with one on the cursor, a click on a listed item in the terminal turns it into an
  `ae2:crafting_pattern` for the first shaped or shapeless crafting recipe that makes that item (ingredients by their first
  listed item). A pattern keeps `{out, out_n, in}` in its custom data.
- `ae2:pattern_provider` holds nine patterns (only patterns go in; menu 9x1). The terminal also lists what the providers'
  patterns make, with a count of 0 when none is stored.
- Middle click on a listed item asks for one: the plan (`src/crafting.rs`) takes what the cells hold and crafts the rest, inputs
  first, up to 16 deep, and says what is missing otherwise. It needs one `ae2:molecular_assembler` with a channel.
  The request is queued as a job and worked off by the assemblers: every 20 ticks each assembler of the network makes
  one craft, each craft costs 200 FE (100 AE) from the cells; a job without power waits, one that runs out of an
  ingredient stops with a message (the power drawn is not refunded). A result no cell has room for goes to the player.
  Jobs are not saved over a restart. Not modelled: crafting CPUs and their storage, per-assembler speed cards,
  processing patterns, amounts other than one.

## Storage bus and interface

- `ae2:storage_bus`: the inventories (item capability of blocks outside the network) next to it become part of the
  network's storage: the terminal lists their contents with the cells', insertion goes to the cells first and then
  to those inventories, extraction takes from the cells first and then from them. No filters or priorities; plain
  stacks only. Needs power and a channel.
- `ae2:interface`: shows the network to its neighbours as an inventory of 54 virtual slots (the network's items, most first);
  a hopper or another mod's bus taking from it takes from the network, anything put in goes into the network. Not
  transactional: a rolled-back outer transaction does not undo what reached the cells. No stocking (AE2's configured
  requests) yet.

## Wireless

- `ae2:wireless_access_point` (a device with a channel, draws 1 AE/tick) and `ae2:wireless_terminal` (item). Sneak-use the
  terminal on an access point to link it (the position is kept in the item's data); using it in the air opens the terminal
  view of that point's network when the player is within 16 blocks of the point and the network has power. Same
  dimension only; no range boost from more points or cards, no battery (AE2WTLib's terminal draws power from the
  item), no other terminal kinds.

## Terminal text search

With the terminal open, typing filters the listing: letters, digits and space build the search text (up to 32
characters), Backspace removes one, the compass slot still sets it from an item. The text is matched against item
names without the namespace, underscores as spaces. While the terminal is open the typing keys are swallowed (E does
not close it); Escape closes it. The text is forgotten when the screen closes. Not AE2's `@mod` or `#tag` syntax,
no search box drawn (the compass slot's name shows the search).

- Speed cards (`ae2:speed_card`) in the bus's slots 1 to 4: 0 to 4 cards move 1, 2, 4, 8 or 16 items per operation (numbers to check against the original).

## Capability for addons: `ae2:network_energy`

Every AE2 block answers `ae2:network_energy` (`mod_ae2::network_energy::NETWORK_ENERGY`, a mod-own `CapabilityKey<dyn EnergyHandler>`
that another crate declares with the same name: `CapabilityKey::new("ae2:network_energy")`): the energy of the block's whole network
as one handler (amount and capacity = the cells', insert fills them, extract drains them). It is not `horizon:energy`,
so other mods' cables do not drain a network by touching a cable. `ae2:patterns` is not built (nothing needs it yet).
