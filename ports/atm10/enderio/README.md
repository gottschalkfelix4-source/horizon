# Ender IO (Horizon port)

Port of [Ender IO](https://github.com/Team-EnderIO/EnderIO) 8.2.12-beta (ATM10 8.x, Unlicense) as a
native Horizon mod. Written from the mod's observed behaviour and public description, not from its
code. The first slice is the **Alloy Smelter**, the machine every Ender IO material comes from.

## What it does

`enderio:alloy_smelter` has a block entity with five slots (three inputs, one output, one
capacitor; for now a hopper's menu shows them) and a 100 000 µI energy buffer the game keeps and
answers through `horizon:energy` from every side (1 000 µI per insert). Each tick it finds the
alloy recipe its three inputs make (shapeless, any slot order; a slot with an item no input uses
blocks the recipe), spends 20 µI/t times its capacitor's factor until the recipe's energy is
paid, then takes the inputs and adds the alloy to the output. No capacitor, no work. Hoppers and
pipes reach the inputs and the output by the container's faces.

| Feature | Status |
|---|---|
| Alloy Smelter: alloy recipes, energy, capacitor speed | ported, not yet checked against the original |
| Alloy Smelter: vanilla smelting mode, its own screen with a progress bar, redstone control, side config | not yet |
| Capacitors (basic, double-layer, octadic) as speed factors | partly: factor only; no capacitor loot data, no energy/capacity scaling |
| Other machines (SAG Mill, Slice'n'Splice, Soul Binder, Vat, Painter, Powered Spawner...) | not yet |
| Generators (Stirling, Combustion, Solar panels) and Capacitor Banks | not yet |
| Conduits (item, fluid, energy, redstone, ME) | not yet |
| Dark Steel tools and armour with upgrades | not yet |
| Travel Anchors and Staff of Travelling, Grains of Infinity | not yet |

## To check against the NeoForge original (oracle on a machine with the jar)

All of these are from memory:

1. The recipe list, its inputs and counts, and each recipe's energy (2 400 to 8 000 µI here).
   In the original they are data-pack recipes (`enderio:alloy_smelting`); here a table in code.
2. The machine's base use (20 µI/t), the capacitor factors (1, 2, 3), the buffer (100 000 µI)
   and its input rate (1 000 µI), and whether a better capacitor also raises the energy per
   recipe or the buffer.
3. Whether a stray item in an input slot really blocks a recipe, and whether an empty capacitor
   slot stops the machine.
4. Item and block ids (`enderio:silicon`, `enderio:*_ingot`, `enderio:*_capacitor`,
   `enderio:alloy_smelter`) and the blocks' facts. `data/enderio/horizon/` holds placeholders until
   `horizon-port-pack carry` brings the jar's own.

## Needs from Horizon

- None blocking this slice. A capacitor-only slot can already be fenced with `menu_slot_rules`
  (players) and the container's faces (hoppers); the alloy recipes as data could go through
  `recipe_serializers` once ported. Both are left for a later slice.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
