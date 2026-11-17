# More Industrial Foregoing Addons (Horizon port)

Port of More Industrial Foregoing Addons (MIFA) 2.1.0 (ATM10 8.x, MIT) as a native Horizon mod. Written
from the mod's observed behaviour and public description, not from its code.

**Confidence is low.** I could not recall MIFA's block list with enough certainty. This folder holds one
generator in the spirit of an Industrial Foregoing add-on, under a name of my choosing, so the mod has a
working, tested piece. It is not a claim about what the original adds.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| Lava Generator (`mifa:lava_generator`) | 8,000 mB tank (fills through `FLUIDS`, burns only `minecraft:lava`), burns 5 mB/t for 120 FE/t (a bucket: 200 ticks, 24,000 FE) while its 500,000 FE buffer has room; pushes up to 2,000 FE/t into each neighbour | stand-in, to check |
| Everything MIFA actually adds | | not yet (not recalled) |
| Recipes, models, lang beyond the block's name | | not yet |

## Needs from Horizon

- A mod screen for the tank and buffer.
- A fluid filter on `ModFluidTanks` (accept only some fluids): today any fluid can be put in the tank and just
  never burns.

## To check against the NeoForge original

1. The mod's real block and item list and which ATM10 uses; replace the stand-in with them.
2. If it has generators: fuel, burn rate and output.
3. Interaction with Industrial Foregoing's addons (speed, efficiency, range) and fluids.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
