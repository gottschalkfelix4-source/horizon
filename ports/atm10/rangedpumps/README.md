# Ranged Pumps (Horizon port)

Port of Ranged Pumps 1.3.0 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's observed
behaviour and public description, not from its code. I could not recall the original's exact blocks and
numbers, so this is the one defining mechanic, a pump with a range, with every number assumed.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| Ranged Pump (`rangedpumps:ranged_pump`) | Every 10 ticks, with 1,000 FE and room for a bucket, takes one fluid source block from the square of radius 8 around the column below it, 32 layers deep (nearest and highest first, 256 positions looked at per operation), into a 16-bucket tank. Only the fluid it already holds, or any when empty. 100,000 FE buffer, 2,000 FE/t in | ported, numbers assumed |
| Tank output | Pipes and fluid handlers draw from every face (`FLUIDS`) | ported |
| Further tiers, a range setting, replacing taken sources (cobblestone/air option), the pump's own screen and lang | | not yet |

## Needs from Horizon

- Nothing blocking. A mod screen would show energy and tank; today only pipes see the contents.
- `Level.pickup_fluid` is used with no player (`None`); if a protection hook needs a player for "may build here"
  checks, a fake player for block entities would be wanted.

## To check against the NeoForge original

1. Whether there are several pump blocks (tiers) and their ranges; whether the range is fixed or configurable.
2. Energy per source and per tick, buffer size, operation rate.
3. Whether the pump also takes flowing fluid, or fills the hole it leaves (original may place a replacement block).
4. Whether the area is below the pump only or all around it.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
