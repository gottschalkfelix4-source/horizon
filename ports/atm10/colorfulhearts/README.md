# Colorful Hearts (Horizon port)

Port of [Colorful Hearts](https://github.com/Terrails/colorful-hearts) 10.5.9 (ATM10 8.x, MIT)
as a native Horizon client mod. Written from the mod's observed behaviour and public description,
not from its code.

## What it does

| Feature | Status |
|---|---|
| Health past 20 in one row: each further 20 points drawn over the row in the next colour, from the left | ported |
| The first 20 points use vanilla's sprites (no change at 20 health or less) | ported |
| Absorption in one row of its own above health, layers coloured the same way (first layer vanilla gold) | ported |
| Absorption drawn over the health row instead (setting `absorption_over_health`) | ported |
| Poison, wither and freeze hearts: vanilla sprites for the first layer, one tint per effect, lighter per layer, above | ported, colours to check |
| Hardcore hearts | ported for the first layer (vanilla sprites); coloured layers use the normal shape |
| Armour, food and air rows, regeneration bump, low-health jitter, damage blink, bubble-pop sound | as vanilla (the hook replaces the whole block, so they are redrawn here from `Gui.renderPlayerHealth`) |
| Configurable colour lists per layer and per effect | not yet: fixed colours (see `HEALTH_COLORS`, `ABSORPTION_COLORS`) |
| Colorful Hearts' own heart textures | not yet: coloured layers are fills inside vanilla's container |
| Blink overlay above 20 health | not yet: only drawn for the vanilla layer |

How: `health_hud` (`HealthInputs` → `Vec<GuiDraw>`) returns the whole block, with blink state
kept between frames in the mod as `Gui` keeps it.

## Needs from Horizon

- **Replace only the hearts.** `health_hud` replaces armour, hearts, food and air together, so
  this port carries a copy of vanilla's other rows, and any other mod that answers
  `health_hud` (or AppleSkin's overlay, which assumes vanilla's stacked rows above 20 health)
  cannot combine with it. Proposed: a hook `player_hearts` in
  `crates/horizon-mod-vocab/src/hooks/player_hearts.rs`,
  `fn player_hearts(inputs: &HealthInputs, x_left: i32, y_base: i32) -> Option<HeartRows>`
  (contract `replace`), where `HeartRows { draws: Vec<GuiDraw>, rows: i32, row_height: i32 }`
  tells the engine how tall the hearts are so it places armour above them itself. Also exposing
  that answer (`rows`, `row_height`) to `gui_layer_post` for `minecraft:player_health` would let
  overlays such as AppleSkin's find the hearts.

## To check against the NeoForge original

1. The default colour lists for health, absorption and each effect, and their order.
2. Whether absorption sits in its own row, over health, or after it, by default.
3. Whether the regeneration bump and the low-health jitter move coloured hearts the same way.
4. The look of the coloured hearts (the original ships textures; here a highlight and a darker
   bottom are drawn).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
