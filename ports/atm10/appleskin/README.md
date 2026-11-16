# AppleSkin (Horizon port)

Port of [AppleSkin](https://github.com/squeek502/AppleSkin) 3.0.9 (ATM10 8.x, Unlicense) as a
native Horizon client mod. Written from the mod's observed behaviour and public description, not
from its code.

## What it does

| Feature | Status |
|---|---|
| Food tooltip: hunger shanks (vanilla `hud/food_full`/`food_half` sprites as text objects, `xN` past 10) and saturation | ported; saturation is a gold number line, not AppleSkin's saturation icons |
| Saturation overlay: an outline on each shank, a whole one per 2 saturation, quarters below | ported; drawn with fills, not AppleSkin's texture |
| Held food preview: the shanks and saturation eating it adds, flashing | ported |
| Health restore preview: the health natural regeneration gives back after eating, flashing on the hearts | ported (tick-by-tick `FoodData.tick` simulation) |
| Exhaustion underlay behind the food row | ported; correct only where the client knows the exhaustion (see below) |
| Settings (Options > Mods): the five toggles above | ported |
| Exhaustion/saturation sync from the server (AppleSkin's server half) | not yet |
| Player-specific food values (`FoodValuesEvent` from other mods) | not yet |
| F3 debug lines for hunger, saturation, exhaustion | not yet |
| Hiding the overlay while a mount's hearts replace the food row | not yet |

## Needs from Horizon

- **Exhaustion on a remote client.** Vanilla sends food and saturation to the client
  (`ClientboundSetHealthPacket`) but not exhaustion; AppleSkin's server half sends it with its
  own payload. The port reads `PlayerRef::food(host).exhaustion` and so shows whatever the client
  holds. Either a server half here (side = "both", a payload sent from `player_tick_post` when
  the value changes, read in the client) or an engine-side `PlayerFood::exhaustion` kept in sync
  on the client would do; the first needs nothing from the engine and is the next step.
- **Riding state on the client**, so the overlay skips the food row when a mount's hearts
  replace it: `PlayerRef::vehicle_max_hearts(self, host: &dyn ClientHost) -> Option<i32>`
  (`crates/horizon-mod-vocab/src/host/players.rs`, client area), or the `HealthInputs` the
  `health_hud` hook already carries passed to `gui_layer_pre`/`gui_layer_post` for
  `minecraft:player_health` (`GuiLayerPostEvent::health: Option<&HealthInputs>`).

## To check against the NeoForge original

1. The saturation overlay's exact look and colour, and whether it follows the shanks' jitter.
2. The flashing curve (here 0.25-0.75 alpha, 1.5 s) and the icons it uses for half steps.
3. The tooltip's exact layout (icons for saturation, the threshold for `xN`).
4. Whether the health estimate is the same simulation (gamerule, difficulty and peaceful not
   taken into account here) and which icons it flashes beyond one heart row.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
