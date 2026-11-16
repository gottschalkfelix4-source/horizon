# KeybindsPurger (Horizon port)

Port of KeybindsPurger 1.4.0 (ATM10 8.6, MIT) as a native Horizon client mod. Written from the mod's
observed behaviour and public description, not from its code.

## What it does

A modpack binds far more keys than a player uses, and many clash. KeybindsPurger lets the player purge
the bindings they don't want, so those actions stop taking keys. This port's screen opens from a
**Purge keys** button in the top left corner of the options screens (in a world) or from its key
(`Purge Key Binds`, unbound by default): every binding is listed with its key; a click purges it or
keeps it again. **Purge conflicts** purges each binding whose key an earlier binding (in Controls'
order, vanilla's first) already holds; **Keep all** clears the list. The list is saved as
`horizon-mod-data/keybindspurger/purged.txt`, one binding id per line, editable by hand.

| Feature | Status |
|---|---|
| Pick bindings to purge, saved across sessions | ported |
| Purge every conflicting binding, keeping the first on each key | ported |
| A purged binding stops acting | ported for vanilla's gameplay keys, while no kept binding shares its key (the press is dropped through `key_mapping_pre`) |
| A purged binding is unbound (its key freed for others) | needs API |
| Purge all modded bindings at once | needs API (other mods' bindings are not listed) |
| Purged bindings hidden from Controls | needs API |

Code: `src/purge.rs` (the list, conflicts, when a press drops; unit tests), `src/lib.rs` (screen, hooks),
`src/keys.rs` (the mappings walked). Texts: `assets/keybindspurger/lang/en_us.json`.

## Needs from Horizon

1. **List and rebind every key binding**: the `ClientHost::key_bindings()` and
   `ClientHost::set_key_binding(id, key, modifiers)` proposed in `../controlling/README.md`
   (`crates/horizon-mod-vocab/src/host/client_input.rs`, with `KeyBindingView` in
   `client/key_mapping.rs`). With them the purge unbinds (`key.keyboard.unknown`), reaches other mods'
   bindings, and "purge all modded" is `owner.is_some()`.
2. **Hide a binding from Controls**: `fn set_key_binding_hidden(&mut self, id: &str, hidden: bool)` beside
   them, kept by the game with the binding.
3. As for Controlling: an options page id (`options.key_binds`) for a button on the right page, and a way
   to open a mod screen from a screen hook.

## To check against the NeoForge original (on a machine with the jar)

1. Whether purging unbinds, hides, or both, and whether a purge is undone on uninstall.
2. Its config's name, format and defaults (any built-in list of bindings it purges by default).
3. Which binding of a conflicting pair it keeps.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
