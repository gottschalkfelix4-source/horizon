# KeyBind Bundles (Horizon port)

Port of KeyBind Bundles 1.4.0 (ATM10 8.6, MIT) as a native Horizon client mod. Written from the mod's
observed behaviour and public description, not from its code.

## What it does

Several actions share one key. Each of the eight bundles has its own key in Controls (`Bundle 1` to
`Bundle 8`, under KeyBind Bundles, unbound by default). Holding a bundle's key opens a radial menu of the
bundle's bindings around the screen's centre; letting go of the key (or clicking) fires the binding under
the pointer, exactly as a press of its own key would. Letting go in the centre fires nothing. A bundle
with nothing in it opens the editor instead (also on the `Edit Bundles` key): click a binding to add it to
the bundle or take it out; `<` and `>` go through the bundles. Bundles are saved as
`horizon-mod-data/keybindbundles/bundles.txt` (`1 = key.drop, key.swapOffhand`).

| Feature | Status |
|---|---|
| A key per bundle, radial menu while held, fire on release or click | ported |
| Bundles of vanilla's movement, gameplay and inventory bindings (attack, use, pick, jump, sneak, sprint, drop, inventory, swap hands, hotbar 1-9) | ported |
| Edit bundles in game, saved across sessions | ported |
| Bundles of other bindings (chat, screenshot, perspective, other mods' keys...) | needs API |
| Named bundles, any number of them, icons | not yet (eight numbered bundles; a key mapping can't be registered after startup) |

Code: `src/bundles.rs` (bundles, saved form, radial geometry; unit tests), `src/lib.rs` (keys, screens).
Texts: `assets/keybindbundles/lang/en_us.json`.

## Needs from Horizon

1. **Fire any binding.** `ClientHost::click_mapping` refuses mappings outside the movement, gameplay and
   inventory categories (`MappingError::NotGameplay`) and knows no mod's mapping. Proposed, in
   `crates/horizon-mod-vocab/src/host/client_input.rs`:
   ```rust
   /// Side: client. Presses binding `id` (`key.screenshot`, `key.togglePerspective`, or `<mod id>:<id>`)
   /// once, as a press and release of its key does with no screen open: the game's own key handler for a
   /// vanilla mapping (`handleKeybinds`, the window shortcuts), one click for a mod's.
   fn press_binding(&mut self, id: &str) -> Result<(), MappingError>;
   ```
   with the `key_bindings()` listing proposed in `../controlling/README.md` so the editor lists them all.
2. **Key mappings registered at run time** (`ClientHost::register_runtime_key_mapping(KeyRegistration)`,
   kept by mod id like the others), for bundles the player adds and names.

## To check against the NeoForge original (on a machine with the jar)

1. Whether the menu fires on release, on click, or both, and what releasing in the centre does.
2. How bundles are made and named (a screen in Controls?), how many there can be, and their save format.
3. The radial's look: slice order (clockwise from the top here), labels, icons, the centre.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
