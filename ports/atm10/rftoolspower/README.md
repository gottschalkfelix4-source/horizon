# RFToolsPower (Horizon port)

Port of RFToolsPower 1.21-7.0.6 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's
observed behaviour and public description, not from its code.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| Dimensional Cells (`cell1`, `cell2`, `cell3`) | Hold a quarter, one and four times 1,000,000 RF; each of six faces is none / input / output (click to cycle); touching cells level out to the same fill so a block acts as one battery | ported |
| Coal Generator | Burns coal or charcoal (600 ticks, 60 RF/t = 36,000 RF), coal blocks ten times as long, other fuels in proportion; 500,000 RF buffer, pushes up to 1,000 RF/t per face; redstone mode (ignored / on / off); a full buffer stops eating fuel | ported |
| Endergenic Generator | Pulse starts charging (1,000 RF/t), a pearl arriving at a charged generator is held, the next pulse fires it at the linked generator; RF paid by how often the pearl was passed (table in `src/endergenic.rs`); lost at an idle generator or after 100 ticks held | ported |
| Pearl Injector | Puts new pearls into the endergenic generator it faces | ported, see below |
| Power Monitor | Reads the energy of the adjacent block, emits a redstone signal below or above a threshold, six-step fill display | ported |
| Configuration GUIs (cell face screen, generator and monitor screens), generator and cell models and animations, the glowing/lit look | | not yet (client) |
| Recipes, lang | | not yet (carried from the pack) |

## Needs from Horizon

- Mod menus for configuration (monitor threshold and mode, redstone mode); today only click-cycling is possible.
- An ender pearl entity with a mod-controlled path, so the flight between generators is drawn; today the flight is
  a countdown saved in the block entity (`incoming`).
- A client block entity renderer for the monitor's level display.

## To check against the NeoForge original

1. Cell capacities (1M normal, x4 advanced, /4 simple?) and the per-face transfer rate (`NORMAL_RF_PER_TICK`);
   whether tiers share the same rate ratio. Which tier is which of `cell1`..`cell3`.
2. Whether cells really level out with touching cells, and how fast.
3. Coal generator numbers: 60 RF/t, 600 ticks per coal, 500,000 RF, 1,000 RF/t out per face.
4. Endergenic: charge draw, `RF_PER_HIT` table, hold timeout 100 ticks, flight speed, how the target is linked.
5. Power Monitor alarm semantics (at or over versus under) and the six display steps.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
