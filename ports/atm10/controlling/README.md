# Controlling (Horizon port)

Port of [Controlling](https://github.com/jaredlll08/Controlling) 19.0.5 (ATM10 8.6, MIT) as a native
Horizon client mod. Written from the mod's observed behaviour and public description, not from its code.

## What it does

Controlling puts a search box and filter buttons on vanilla's Key Binds screen. Horizon's mod API cannot
change that screen, so this port shows the same list, with rebinding, on a screen of its own,
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
| The list over vanilla's own Key Binds screen | needs API (own screen instead) |
| Click a key button, then press a key or mouse button to rebind; Escape unbinds | ported (a mod's binding keeps Shift/Ctrl/Alt) |
| Reset one binding (per-row Reset, shown when off its default) | ported |
| Reset All | ported |
| Other mods' bindings in the list | ported (`key_bindings`; conflicts use key, modifiers and conflict context) |

Code: `src/search.rs` (conflicts, search, filters, sort, free keys, rebinding and Reset All plans; unit tests), `src/keys.rs` (the
keyboard keys walked), `src/lib.rs` (the screen). Texts: `assets/controlling/lang/en_us.json`.

## Needs from Horizon

1. ~~Every key binding, readable and writable~~ **done**: `key_bindings` and `set_key_binding` list and
   rebind every binding. The screen hooks hold the host read-only, so a rebind is queued and made on the
   next client tick.
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
5. Rebinding details: whether Escape unbinds, whether modifiers are kept, and whether a conflicting rebind asks.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
