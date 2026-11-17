# Mekanism Generators (Horizon port)

Port of Mekanism Generators 10.7 (ATM10 8.x, MIT) as a native Horizon mod beside the
[Mekanism port](../mekanism/). Written from observed behaviour, not from the mod's code.

| Part | Behaviour | Status |
|---|---|---|
| Solar / Advanced solar generator | 50 / 300 J per tick (20 / 120 FE) by day under open sky, nothing at night or in rain | ported |
| Wind generator | Output rises with height from nothing at y 24 to 480 J/tick at y 319 | ported, curve is a stand-in |
| Bio generator | Burns bio fuel (200 ticks) and crops/seeds (100 ticks) at 350 J/tick | ported, fuels to check |
| Heat generator | Burns any furnace fuel at 200 J/tick for its burn time | partly: no lava tank, no heat from the surroundings |
| Gas-burning generator, turbine, fission and fusion reactors, boiler, induction matrix | | not yet (need the multiblock and gas machines) |

Generators hold 100 ticks of output, give to adjacent energy blocks (up to 4x their output) and the
Mekanism port's universal cables draw from them. Fuel goes in through the hopper menu (slot 0).

## To check against the NeoForge original

1. Every number in `src/rules.rs` (outputs, buffer, hand-on rate, wind curve, fuel list and burn times).
2. The heat generator's lava and environment heat, the bio generator's fuel list.
3. Hardness and resistance (`gen_data.py` writes placeholders until `horizon-port-pack carry`).

## Build

```sh
sh ../dev-sdk.sh && cargo test
```
