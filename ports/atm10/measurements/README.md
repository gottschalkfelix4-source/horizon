# Measurements (Horizon port)

Port of Measurements 3.0.3 (ATM10, MIT) as a native Horizon mod. Written from the mod's observed
behaviour and public description, not from its code.

## What it does

| Feature | Behaviour | Status |
|---|---|---|
| `measurements:tape_measure` | One-per-stack item | ported (placeholder item facts) |
| Pick the start | Right-click a block: it becomes the start (kept on the player, not saved) | ported |
| Measure | Right-click a second block: the action bar shows the box's size along x, y and z (both ends counted), its block count, and the straight distance between the two blocks' centres; the start is cleared | ported, not yet checked against the original |
| Clear | Sneak + right-click a block clears the start | ported, assumed |
| The box drawn in the world | The client follows the same clicks and draws the box being measured (blue, the start block) and then the measured box (gold) as lines seen through walls, until the next click | ported, colours assumed |
| Edge lengths as floating labels beside the drawn box | | not yet |
| Recipe, texture, model | | from `horizon-port-pack carry` on the jar |

Messages are translation keys (`measurements.start`, `measurements.result`,
`measurements.cleared`) in `assets/measurements/lang/en_us.json`.

## Needs from Horizon

- Floating labels: `WorldDrawContext` draws lines and quads but no text in the world. Needs
  e.g. `WorldDrawContext::text(&mut self, at: [f64; 3], text: &Text, argb: u32, depth: WorldDepth)
  -> Result<(), DrawError>` (billboarded, as a name tag is drawn).
- The client keeps its own copy of the clicks, so a click the server refuses (out of reach, an
  adventure-mode rule) can leave the drawn box ahead of the server's start; a synced player
  attachment would fix that.

## To check against the NeoForge original (oracle on a machine with the jar)

1. What the original reports: per-axis sizes, area, volume, distance, and its rounding.
2. Whether the measurement shows in chat, the action bar, or only in the world.
3. How a start is cleared (sneak-click, right-click in air, picking a new start).
4. Whether the start survives logout or is per item stack (stored on the stack).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
