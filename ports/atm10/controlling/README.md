# Controlling (Horizon port)

Port of [Controlling](https://github.com/jaredlll08/Controlling) 19.0.5 (ATM10 8.6, MIT) as a native
Horizon client mod. Written from the mod's observed behaviour and public description, not from its code.

## What it does

Controlling puts a search box and filter buttons on vanilla's Key Binds screen. Horizon's mod API can
neither change that screen nor rebind a key, so this port shows the same list on a screen of its own,
opened from a **Search keys** button in the top right corner of the options screens (in a world) or from
its key (`Search Key Binds`, unbound by default, under Controlling in Controls).

| Feature | Status |
|---|---|
| Search box, case-insensitive, over the bindings' names | ported |
| Search by key ("Search: Name / Key / Both") | ported |
| Show: All / Conflicts / Unbound | ported |
| Sort: none (by category, with headings) / A-Z / Z-A | ported |
| Conflicting keys in red | ported; debug keys (F3 + key) only clash with each other |
| Free keys: the keyboard keys no binding uses | ported |
| The list over vanilla's own Key Binds screen, with its rebinding | needs API |
| Click a row to rebind, reset one binding | needs API |
| Reset All | needs API (drawn greyed out) |
| Other mods' bindings in the list | needs API: only vanilla's and Horizon's are listed |

Code: `src/search.rs` (conflicts, search, filters, sort, free keys; unit tests), `src/keys.rs` (the
mappings and keyboard keys walked), `src/lib.rs` (the screen). Texts: `assets/controlling/lang/en_us.json`.

## Needs from Horizon

1. **Every key binding, readable and writable.** In `crates/horizon-mod-vocab/src/host/client_input.rs`
   (`ClientInputClientHost`):
   ```rust
   /// Side: client. Every mapping Controls lists, vanilla's, Horizon's and every mod's, in its order.
   fn key_bindings(&self) -> Vec<KeyBindingView>;
   /// Side: client. Rebinds `id` (`key.jump`, or `<mod id>:<id>`) as Controls does and saves it;
   /// `key.keyboard.unknown` unbinds it.
   fn set_key_binding(&mut self, id: &str, key: &str, modifiers: ClientModifiers) -> Result<(), MappingError>;
   ```
   with `KeyBindingView { id, label: Text, category: String, key: String, modifiers: ClientModifiers,
   default_key: String, default_modifiers: ClientModifiers, context: KeyConflictContext, owner: Option<String> }`
   in `client/key_mapping.rs`. It would list other mods' keys, and turn on rebinding and Reset All.
2. **Know the Key Binds page.** `ScreenView::id` reports `options` for every options page. An id per page
   (`options.key_binds`) would let the search box sit on vanilla's own screen, as Controlling's does, and
   keep the **Search keys** button off the other pages.
3. **Open a mod screen from a screen hook.** The screen hooks hold `&dyn ClientHost`, so the button's click
   is acted on at the next client tick, which closes the options screens before opening the mod's. A
   `ScreenOpening`-style answer from `screen_mouse_button_pressed_pre` (or `mod_screen_open` on `&self`)
   would open it straight over them and return to them on Escape.

## To check against the NeoForge original (on a machine with the jar)

1. The search's exact matching: substring or word start, and whether category names are searched.
2. The buttons' labels, order and cycle, and whether the sort keeps category headings.
3. Which conflicts it marks: NeoForge conflict contexts and key modifiers; the debug keys.
4. The free keys list: which keys it walks, and whether mouse buttons count.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
