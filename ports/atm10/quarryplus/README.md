# QuarryPlus (Horizon port)

Port of QuarryPlus 21.1.164 (ATM10 8.x, LGPL-3.0-only, so this port is LGPL-3.0-only too) as a native
Horizon mod. Written from the mod's observed behaviour and public description, not from its code.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| Marker (`quarryplus:marker`) | Three markers in an L (corner, one along x, one along z, within 64 blocks) mark the area | ported (a plain block; the original's marker also has a GUI/link tool) |
| Quarry (`quarryplus:quarry`) | Touching the corner marker, finds the area between the markers and digs it a layer at a time, from just below itself to the bottom of the world, x fastest. Each block costs 200 + 400 x hardness energy (1,000,000 buffer, 100,000 in per tick); skips air, fluids, bedrock and its own blocks; drops go into inventories beside it (`ITEMS`), else onto the ground above it | ported, numbers assumed |
| Frame (the quarry builds a frame around the area before digging), Pump upgrade (removes fluids), enchantments (efficiency, fortune, silk touch, unbreaking) in the quarry, Mini Quarry, Mover, Placer, Remote Placer, Workbench, Replacer, Filler, area retaining chunk loading, GUI | | not yet |

## Needs from Horizon

- **Fortune and silk touch from a block entity**: `destroy_block` takes a `tool: Option<&ItemStack>`, so an
  enchanted tool stack could be passed, but a block entity has no item to hold the enchantments; a quarry would
  need its enchantments as a virtual tool. Wanted: a documented way to build a throwaway tool `ItemStack` with
  enchantment components, or a `tool_enchantments` argument.
- **Chunk loading for an area** (`set_mod_forced` exists): the quarry waits at unloaded positions rather than
  forcing them.
- **Mod screens** for the quarry's status and settings, and a marker link tool.
- **Block placement during a tick without updates** for building the frame in bulk.

## To check against the NeoForge original

1. Marker rules: whether the area is between the markers or includes them, and the 64-block span.
2. Energy per block, buffer size and maximum input; whether the cost depends on depth or the block's hardness.
3. Dig order and start height (here from the quarry's own height down, layer by layer, not frame-bounded).
4. Whether drops go to any adjacent inventory.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
