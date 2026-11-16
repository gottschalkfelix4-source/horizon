# Dark Mode Everywhere (Horizon port)

Port of [Dark Mode Everywhere](https://github.com/Buuz135/DarkModeEverywhere) 1.4.0 (ATM10 8.6, MIT) as
a native Horizon client mod. Written from the mod's observed behaviour and public description, not from
its code.

## What it does

The original swaps the GUI shaders so every screen's light-grey textures, vanilla's and every mod's, draw
dark, with a button in a screen corner to change the darkness and a blacklist of screens it leaves alone.
Horizon's mod API has no colour filter for the GUI yet, so this port darkens what it can reach: once a
screen has drawn its panel (`screen_render_background`), before its slots, items, widgets and text, it lays
a translucent black over the panel. Items, the hover highlight and tooltips keep their colours. On chests,
barrels, shulker boxes and hoppers it redraws the title and "Inventory" in a light colour over vanilla's
dark-grey ones.

| Feature | Status |
|---|---|
| Container and mod screens' panels darkened, any mod's included | ported (overlay; partial: an overlay darkens, it can't remap colours) |
| Darkness presets cycled by a button in the bottom left corner (Off, Dim, Dark, Darkest) | ported |
| Blacklist of screens (Shift-click the button; by menu type, or by screen id) | ported |
| Choices kept (`horizon-mod-data/darkmodeeverywhere/state.txt`, editable: `blacklist <menu or screen id>`) | ported |
| Container labels readable on the dark panel | ported for chest-like menus; other menus keep vanilla's dark labels |
| Buttons, text fields, sliders and scroll bars drawn dark; screens without a panel (title, options) | needs API |
| Real colour remapping (light greys to dark greys, dark text to light) | needs API |
| Hide the button (setting `Corner button`) | ported |

Code: `src/dark.rs` (presets, blacklist, label spots, saved form; unit tests), `src/lib.rs` (hooks).
Texts: `assets/darkmodeeverywhere/lang/en_us.json`.

## Needs from Horizon

**A colour filter for the GUI.** A hook asked once a frame for the open screen, in a new
`crates/horizon-mod-vocab/src/hooks/gui_color_filter.rs`:
```rust
/// The open screen is about to draw (`screen` as the screen hooks see it): set `filter` to recolour
/// its textured GUI quads (sprites, nine-slices, the panel), and `text_filter` its text, until the
/// screen is drawn. Items, entities and the world are never filtered. Client-side.
#[hook(side = "client", contract = "listen", neoforge = "none (Horizon-only)")]
fn gui_color_filter();
// event: GuiColorFilterEvent<'a> { screen: &'a ScreenView<'a>, filter: &'a mut Option<ColorMatrix>,
//                                  text_filter: &'a mut Option<ColorMatrix> }
```
with `ColorMatrix { rows: [[f32; 5]; 4] }` (RGBA out = matrix × RGBA1 in) in `client/types.rs`, applied in
the GUI pipeline's fragment shader. With it the port keeps light-grey `#C6C6C6` → dark `#2B2B2B`,
dark text `#404040` → light, and buttons and panel-less screens darken too, as the original's shader does.

## To check against the NeoForge original (on a machine with the jar)

1. The shader's colour mapping (which colours it darkens, what text becomes), and its presets' names.
2. The button's corner, look and what Shift-click or a right click does; the default blacklist (screens of
   mods known to break) and the config's name and format.
3. Whether it darkens the title and options screens, tooltips, the HUD.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
