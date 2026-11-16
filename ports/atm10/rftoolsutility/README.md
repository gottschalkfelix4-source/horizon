# RFToolsUtility (Horizon port)

Port of RFToolsUtility 1.21-7.0.12 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's
observed behaviour and public description, not from its code. The mod is large; this port covers the
redstone logic blocks and the Environmental Controller.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| Timer | Output on for 1 tick at the start of every `delay` ticks (default 20); an empty-hand click steps the delay (1, 2, 5, 10, 20, 40, 100, 200, 600) | ported, set by click instead of a screen |
| Counter | Counts rising edges of the redstone input; on the target (default 4, click steps 1..64) it resets and outputs for 2 ticks | ported, to check |
| Redstone Transmitter / Receiver | Wireless: a transmitter reports the strongest signal at its sides on its channel (0..15, click steps it); a receiver on the same channel outputs the strongest fresh report (any dimension) as an analog signal. A transmitter silent for 5 ticks stops counting | ported |
| Environmental Controller | RF buffer 1,000,000 (20,000 in per tick); nine module slots (vanilla 9x1 menu); every 10 ticks, if it holds the RF for them, it pays and renews 40-tick effects on players in a cube of its radius (10 to 100, default 50, click steps it). Modules: speed II, haste II, regeneration, night vision, water breathing, slow falling (feather falling), glowing, saturation. Cost: the modules' RF/t added up (10 to 40 each), scaled linearly with the radius over 50 | ported, numbers assumed |
| Crafters, Spawner, Matter Transmitter/Receiver/Dialer, Matter Beamer, Tank, Sequencer, Logic block, Sensor, Inventory Checker, Screens and their modules, Button, Item/Fluid/Energy... | | not yet |
| Environmental modules beyond the eight (peaceful, flight, blindness, slowness, poison, weakness, wither, hunger, ...) | | not yet |

## Needs from Horizon

- **Mod screens** with number fields and toggles, to replace the click-to-step settings, and the screens the
  rest of the mod is built on (crafter recipes, screens with modules, matter transmitter dialer).
- **Peaceful / flight modules** need `LivingRef` to set mob targets off or `abilities` on a timer from a block
  entity; flight itself (`set_abilities`) exists but must be reverted when the player leaves the range, which
  needs a player-tick or area-leave hook.
- **A world-level (not per-chunk) place to keep what the transmitters report** that survives a restart:
  today an in-memory table with a 5-tick expiry, so a receiver reads 0 until its transmitter has ticked after a
  load.

## To check against the NeoForge original

1. Timer: default delay, pulse length, whether the delay is configurable with a pause time.
2. Counter: whether it holds its output while at or over the target, and the pulse length.
3. Transmitter / Receiver: the channel range (here 0..15) and whether receivers see other dimensions.
4. Environmental Controller: energy buffer, per-module RF/t and how cost scales with the area; whether the area
   is a cube around the block or a column with min/max Y; amplifier of each effect; the module list.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
