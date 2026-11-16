# Modern Industrialization (Horizon port)

Port of [Modern Industrialization](https://github.com/AztechMC/Modern-Industrialization) 2.5.10
(ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's observed behaviour and public
description, not from its code. The first slice is the **Bronze Boiler**, the machine the steam
age runs on.

## What it does

`modern_industrialization:bronze_boiler` has a block entity with a fuel slot (for now a furnace's
menu shows it: the flame is the fuel left, the arrow the temperature) and two 16 000 mB tanks the
game keeps and answers through `horizon:fluids`: water in, steam out. It lights a new piece of
furnace fuel only when it has water and room for steam (a lava bucket leaves its bucket), heats
by 1 °C per tick while burning up to 1 100 °C, cools by 1 °C per tick otherwise, and from 100 °C
boils water into steam, up to 8 mB/t at its hottest, in proportion to the temperature (1 mB of
water makes 1 mB of steam). A pipe or another mod's machine drains the steam.

| Feature | Status |
|---|---|
| Bronze Boiler: fuel, temperature, water to steam | ported, not yet checked against the original |
| Steel Boiler, Large Steam Boiler, Steam Turbine | not yet |
| Steam machines (Bronze Furnace, Macerator, Compressor, Mixer, Cutting Machine...) | not yet |
| Electric machines, cables and voltage tiers (LV to Superconductor) | not yet |
| Multiblocks (Coke Oven, Blast Furnace, Distillation Tower, Fusion Reactor, Nuclear Reactor...) | not yet |
| Item and fluid pipes with their configuration | not yet |
| Materials: ores, dusts, plates, ingots, forge hammer | not yet |
| The machine's own screen, output auto-push, wrench and side config | not yet |

## To check against the NeoForge original (oracle on a machine with the jar)

All of these are from memory:

1. The boiler's temperatures (boiling at 100 °C, max 1 100 °C), heating and cooling per tick,
   and the steam curve (linear in temperature here, 8 mB/t at max).
2. Fuel burn time: here the furnace burn time (`BURN_DIVISOR` = 1); the original may burn faster.
3. Whether it burns fuel without water or with a full steam tank (here it doesn't start a new one).
4. Tank sizes (16 000 mB each), the steam fluid id `modern_industrialization:steam`, the block
   id and its facts: `data/modern_industrialization/horizon/` holds placeholders until
   `horizon-port-pack carry` brings the jar's own.

## Needs from Horizon

- `ModFluidTanks::new`: `ModFluidTanks` is `#[non_exhaustive]` with no constructor, unlike
  `ModContainer` and `ModEnergyBuffer`, so a mod builds it from `Default` field by field. Proposed,
  in `crates/horizon-mod-vocab/src/item/containers.rs`:
  `impl ModFluidTanks { pub fn new(tanks: Vec<i32>, faces: [Vec<usize>; 6], inputs: Vec<usize>, outputs: Vec<usize>) -> Self }`.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
