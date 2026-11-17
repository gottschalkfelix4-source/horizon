# Extreme Reactors (Horizon port)

Port of Extreme Reactors 1.21.1-2.4.28 (ATM10 8.x, MIT) as a native Horizon mod. Written from the
mod's observed behaviour and public description, not from its code. Only the passive reactor is
ported: no turbines, no coolant, no reprocessor, no fuel mixes.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| Multiblock check | The controller looks at the connected reactor parts around it each second: a hollow cuboid, 3 to 32 on a side, casing on the edges, casing, the controller and power taps on the faces, control rods on the top face over full columns of fuel rods, only fuel rods and air inside | ported (the port's own rules) |
| `reactor_fuel_rod`, `reactor_control_rod`, `reactor_casing`, `reactor_power_tap`, `reactor_controller` | Plain blocks; only the controller has a block entity | ported |
| Fuel | Yellorium ingots in the controller's slot 0 (1000 units each, 4000 units of room per fuel column); waste comes out as cyanite ingots in slot 1 per 1000 units | ported, numbers assumed |
| Heat and energy | Each rod makes 2 heat per tick at full activity and burns 0.01 units; the casing sheds 2 % of the heat above 20 each tick as 10 FE per heat, so a rod yields 20 FE per tick at steady state | ported, numbers assumed |
| Control rods | Using the controller steps the insertion 0, 10, ..., 100; activity is the rest | ported (a button in the original's screen) |
| Power taps | The controller pushes its 1 000 000 FE buffer into every neighbour of a tap that takes energy | ported |
| Redstone | A signal at the controller stops the reaction | assumed |
| Active cooling, turbines, coolant ports, fluid fuel, access ports, reactor glass, computer ports, wastes, reprocessor, meltdown | | not yet |

## Needs from Horizon

1. Mod menu screens with a progress/heat/energy display and control-rod buttons (vanilla's hopper menu is used for the two item slots).
2. Multiblock helpers: a block-entity "part registered" notification so a controller revalidates when one of its parts is placed or broken; today it polls every second.
3. A way for a block without a block entity (a power tap, an access port) to forward `ENERGY` and `ITEMS` capabilities to its controller (`block_capability` can, but needs the controller's position, which the port would have to store per part).

## To check against the NeoForge original

1. Every number in `src/rules.rs`: fuel per ingot, fuel per rod per tick, heat per rod, cooling rate, FE per heat.
2. The structure rules: whether control rods go on the top face only, whether interior holes are allowed, the frame, the size limits.
3. Whether yellorium is the only fuel and how waste leaves the reactor.
4. Whether a passive reactor can overheat, and what a redstone signal does.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
