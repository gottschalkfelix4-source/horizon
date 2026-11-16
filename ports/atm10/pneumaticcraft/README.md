# PneumaticCraft: Repressurized (Horizon port)

Port of [PneumaticCraft: Repressurized](https://github.com/TeamPneumatic/pnc-repressurized) 8.2.23
(ATM10 8.x, LGPL-2.1-only; this port is LGPL-2.1-only too) as a native Horizon mod. Written from
the mod's observed behaviour and public description, not from its code. The first slice is
**pressure**: making compressed air, moving it, and what too much of it does.

## What it does

Every pneumatic block keeps its air in its block entity (`air`, mL above atmospheric; pressure in
bar = air / volume). The `pneumaticcraft:air_compressor` (5 000 mL) burns furnace fuel from its
fuel slot (for now a furnace's menu shows it: the flame is the fuel left, the arrow the pressure
up to critical) and adds 10 mL of air per tick while burning. `pneumaticcraft:pressure_tube`
(1 000 mL) holds air and passes it on. Each tick a block pushes air into every neighbouring
pneumatic block at a lower pressure, so that both end at the same pressure; air is conserved and
never sloshes back and forth; a vacuum (down to -1 bar) pulls air in the same way. Above the
danger pressure (5 bar) a block may explode, with a chance per tick rising towards critical
pressure (7 bar) and drawn from the level's random; above critical it explodes.

| Feature | Status |
|---|---|
| Air, volume, pressure; equalising between neighbouring pneumatic blocks | ported, not yet checked against the original |
| Danger and critical pressure explosions (tier one) | ported, chance curve from memory |
| Air Compressor (burns fuel) | ported |
| Pressure Tube | ported |
| Advanced (tier two) tubes and compressors, Reinforced/Advanced Pressure Tubes, Volume Upgrades, Security Upgrades | not yet |
| Safety tube modules, pressure gauges, regulator modules, Vacuum Pump | not yet |
| Pressure Chamber multiblock and its recipes | not yet |
| Thermal (heat) system, Refinery, Thermopneumatic Processing Plant, plastics | not yet |
| Programmable Drones, Programmer, Elevator, Assembly Line, Pneumatic Armor, Minigun | not yet |
| Machines' own screens, the in-game manual (PNC:R Manual) | not yet |

## To check against the NeoForge original (oracle on a machine with the jar)

All of these are from memory:

1. Volumes (compressor 5 000 mL, tube 1 000 mL), the compressor's 10 mL/t and whether its burn
   time is the furnace's.
2. Danger 5 bar and critical 7 bar for tier one; the explosion's chance in between (here linear up
   to 0.5 % per tick), its radius (1.0) and whether it breaks blocks.
3. How air spreads: here pairwise equalising in the order down, up, north, south, west, east,
   each tick, within one tick; the original may spread a share per tick instead.
4. Block ids and facts: `data/pneumaticcraft/horizon/` holds placeholders until
   `horizon-port-pack carry` brings the jar's own.

## Needs from Horizon

- A quiet write of another block entity's saved data. Pushing air into a neighbour goes through
  `BlockEntityRef::merge_data`, which also sends clients the block's update tag
  (`sendBlockUpdated`); a pressure network changes air every tick, so every tube's update goes out
  every tick. Proposed, in `crates/horizon-mod-vocab/src/host/block_entities.rs`
  (`BlockEntitiesBackend`, server side, transactional like `merge_data`, handle method
  `BlockEntityRef::merge_data_quiet`):
  `fn block_entity_merge_data_quiet(&mut self, entity: BlockEntityRef, data: Compound) -> Result<bool, BlockEntityError>`:
  merge and mark the chunk changed (`setChanged`), without `sendBlockUpdated`.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
