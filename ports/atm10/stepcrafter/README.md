# Step Crafter (Horizon port)

Port of Step Crafter 1.21.1-0.1.9 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's
public description, not from its code.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| `stepcrafter:step_crafter` | Dropper-template block (six-way `facing`, `triggered`), 9 slots in vanilla's 3x3 menu, hoppers and pipes reach every slot | ported |
| Crafting | On each rising redstone edge: match the 3x3 grid against `minecraft:crafting` recipes through the engine, take one from every occupied slot, remainders (a bucket's bucket) go into the emptied slot or are dropped | ported, not yet checked against the original |
| Output | Pushed into the storage the block faces through `ITEMS`; what does not fit, and spilled remainders, drop in front | ported |
| Recipe ghost/filter slots, comparator output, GUI with output preview | | not yet |

## Needs from Horizon

1. Mod menu screens with a result preview slot (today the vanilla 3x3 dispenser menu is shown, with no preview).

## To check against the NeoForge original

1. Whether it crafts on a rising edge or on each tick while powered, and whether the grid is a fixed pattern (a ghost recipe) or whatever the items form.
2. Where the result goes when nothing takes it (dropped in front, as assumed here, or kept in an output slot).
3. Whether it consumes ingredients one at a time per pulse or per whole stack.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
