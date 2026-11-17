# KeyBind Bundles (Horizon port)

Port of KeyBind Bundles 1.4.0 (ATM10 8.6, MIT) as a native Horizon client mod. Written from the mod's
observed behaviour and public description, not from its code.

## What it does

Several actions share one key. Each of the eight bundles has its own key in Controls (`Bundle 1` to
`Bundle 8`, under KeyBind Bundles, unbound by default). Holding a bundle's key opens a radial menu of the
bundle's bindings around the screen's centre; letting go of the key (or clicking) fires the binding under
the pointer, exactly as a press of its own key would. Letting go in the centre fires nothing. A bundle
with nothing in it opens the editor instead (also on the `Edit Bundles` key): click any binding (the game's,
Horizon's or another mod's) to add it to the bundle or take it out; `<` and `>` go through the bundles and
`+` adds one (up to 16). Bundles are saved as
`horizon-mod-data/keybindbundles/bundles.txt` (`1 = key.drop, key.swapOffhand`).

| Feature | Status |
|---|---|
| A key per bundle, radial menu while held, fire on release or click | ported |
| Bundles of any binding (chat, screenshot, perspective, other mods' keys...), fired through `press_binding` | ported |
| Edit bundles in game, saved across sessions | ported |
| More bundles than eight (`+` in the editor; their keys are registered at run time) | ported, up to 16; a bundle can't be removed again |
| Named bundles, icons | not yet (a key's label is a translation key, so bundles are numbered) |

Code: `src/bundles.rs` (bundles, saved form, radial geometry; unit tests), `src/lib.rs` (keys, screens).
Texts: `assets/keybindbundles/lang/en_us.json`.

## Needs from Horizon

1. ~~Fire any binding~~ **done** (`press_binding`, with `key_bindings` for the editor's list).
2. ~~Key mappings registered at run time~~ **done** (`register_runtime_key_mapping`), used for bundles 9 to 16.
3. **Unregister a runtime key mapping** (and one with a label that is not a translation key), so a bundle can
   be removed or named by the player: e.g. `fn unregister_runtime_key_mapping(&mut self, id: &str)` and a
   `KeyRegistration` label taking a `Text`.

## To check against the NeoForge original (on a machine with the jar)

1. Whether the menu fires on release, on click, or both, and what releasing in the centre does.
2. How bundles are made and named (a screen in Controls?), how many there can be, and their save format.
3. The radial's look: slice order (clockwise from the top here), labels, icons, the centre.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
