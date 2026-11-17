# AE2 Import Export Card (Horizon port: planned)

What it does in ATM10 (from its public description): adds an **import card** and an **export card** that are
inserted as upgrades (into the AE2 terminals / storage blocks that accept upgrades) to let a block move items
between the network and a neighbouring inventory without an import or export bus.

Where the [AE2 port](../ae2/) stands for it:

- **Already there:** the network's storage operations (`network_insert` / `network_extract`), import and export buses
  with a filter slot, the storage bus and the interface.
- **Not there:** upgrade cards for AE2 blocks (AE2's upgrade slots on buses, the interface and the drive are not
  modelled), so there is nothing for the cards to plug into yet. Once the buses and the interface have an upgrade
  inventory, the two cards are two items with a rule in the bus tick (speed and a filter-free mode).

Licence of the original: MIT (docs/modpacks/atm10-mods.csv), so a committed port is allowed.
