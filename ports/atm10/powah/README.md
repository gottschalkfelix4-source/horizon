# Powah (Horizon port)

Port of [Powah](https://github.com/owmii/powah) 6.2.10 (ATM10 8.x, LGPL-3.0-only) as a native Horizon
mod. Written from the mod's observed behaviour and public description, not from its code.

## What it does

Blocks are `powah:<kind>_<tier>` in seven tiers (starter, basic, hardened, blazing, niotic, spirited,
nitro); the energizing orb has none. Each kind has one block entity type; energy is the game's
`ModEnergyBuffer` (saved `energy`), so other mods' cables and machines reach it through `ENERGY`.

| Part | Behaviour | Status |
|---|---|---|
| Energy cells | Buffer per tier; give to neighbours that are not Powah blocks (cables pull from them) | ported |
| Energy cables | A connected network (up to 1024 cables) run by its lowest cable: gathers what generators pushed in, feeds machines up to the adjacent cable's rate, tops machines up from cells, stores the rest in cells | ported, network model is the port's own |
| Furnator | Burns any furnace fuel from 5 slots (hopper menu): 30 FE per tick of burn time, at the tier's rate; takes fuel only with room | ported |
| Magmator | Lava tank (5 B, buckets by hand or pipes): 10 FE per mB at the tier's rate | ported |
| Thermo generator | Heat source below (lava source 1000, magma 800, blazing crystal block 2800) × tier rate / 1000, spending 1 mB water a tick | ported |
| Solar panel | Tier rate by day under open sky, nothing at night or in rain | ported |
| Reactor | Single block: uraninite fuel, coal/charcoal +20 % output, redstone −20 % fuel use, water cools; overheats at 1000° and restarts at 900°; a redstone signal stops it | partly: the 3×4×3 formed structure is not built |
| Energizing rods and orb | The orb (3×3 menu, last slot the result) matches a recipe and draws from every rod within 4 blocks at the rod's rate until the recipe's energy is in | ported; recipes hard-coded (5) |
| Ender cells | Per owner, 12 channels (sneak-use with an empty hand cycles); every cell of a channel shares its energy | ported per level (see Needs) |
| Generators show `lit` while making energy | | ported |
| Creative tier, capacitors and upgrades, wrench side config, ender gate, player transmitter, charging, discharger, energy hopper, dielectric casing | | not yet |
| Screens showing energy, heat, fuel | | not yet (vanilla menus have no energy bar) |

## Needs from Horizon

1. A world-wide saved-data holder (NeoForge's `SavedData` on the overworld's data storage) for
   ender channels: `Holder::Level` stores them per dimension, so an ender cell in the Nether does not
   share with one in the Overworld as in Powah.
2. A data-driven recipe type for `powah:energizing` with an `energy` field that `LevelRef::recipe_for`
   matches as an unordered multiset of up to 6 ingredients: today the recipes are a table in
   `src/rules.rs` and datapacks cannot add or change them.
3. Mod menu screens (a client-side `screen` for a menu type) able to draw an energy bar from data
   slots; until then the machines open vanilla menus and their energy is not visible.
4. A multiblock placement helper (one item that places a structure of 36 linked blocks and breaks it
   together) for the reactor's 3×4×3 form.

## To check against the NeoForge original

1. Every tier number in `src/rules.rs` (`CELL_CAPACITY`, `CELL_TRANSFER`, `CABLE_TRANSFER`, `SOLAR`,
   `FURNATOR`, `MAGMATOR`, `THERMO`, `REACTOR`, `ROD`): from the default config as remembered.
2. Furnator energy per fuel tick (30 FE) and magmator FE per mB of lava (10).
3. The list of heat sources and their heat, and whether thermo generators accept other coolants and
   how much they spend.
4. The reactor's whole model (fuel per uraninite, carbon and redstone effects, temperatures,
   coolant use), and how the formed structure behaves.
5. The energizing recipes, their energy and result counts; the rods' range and whether a rod must
   face the orb; whether extra items block a recipe.
6. Ender cell capacity per tier and channel count (12), and how channels are chosen.
7. Whether cells push into every side by default and how cables split energy between receivers.
8. Hardness and blast resistance; the `data/powah/horizon/block/*.json` files are placeholders until
   `horizon-port-pack carry` reads them (and the recipes, models, loot tables) from the jar.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
