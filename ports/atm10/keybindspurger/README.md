# KeybindsPurger (Horizon port)

Port of KeybindsPurger 1.4.0 (ATM10 8.6, MIT) as a native Horizon client mod. Written from the mod's
observed behaviour and public description, not from its code.

## What it does

A modpack binds far more keys than a player uses, and many clash. KeybindsPurger lets the player purge
the bindings they don't want, so those actions stop taking keys. This port's screen opens from a
**Purge keys** button in the top left corner of the options screens (in a world) or from its key
(`Purge Key Binds`, unbound by default): every binding, other mods' included, is listed with its key; a
click purges it or keeps it again. A purge unbinds the binding and hides it from Controls; keeping it
again shows it and puts its key back. **Purge conflicts** purges each binding whose key (and modifiers)
an earlier binding in Controls' order, vanilla's first, already holds; **Purge modded** purges every
binding a mod registered (this mod's own key stays); **Keep all** clears the list. The list is saved as
`horizon-mod-data/keybindspurger/purged.txt`, one binding id per line, optionally followed by the key it
had (`id<TAB>key<TAB>modifiers`); editable by hand. A purged binding the player binds again is unbound
again within two seconds.

| Feature | Status |
|---|---|
| Pick bindings to purge, saved across sessions | ported |
| Purge every conflicting binding, keeping the first on each key | ported |
| A purged binding is unbound (its key freed for others) | ported (`set_key_binding`) |
| Purge all modded bindings at once | ported (`owner.is_some()`) |
| Purged bindings hidden from Controls | ported (`set_key_binding_hidden`) |
| Keep a purged binding again, restoring its key | ported |

Code: `src/purge.rs` (the list, conflicts, the unbind/hide/restore steps; unit tests), `src/lib.rs` (screen,
hooks). Texts: `assets/keybindspurger/lang/en_us.json`.

## Needs from Horizon

1. ~~List and rebind every key binding; hide a binding~~ **done** (`key_bindings`, `set_key_binding`,
   `set_key_binding_hidden`). The screen hooks hold the host read-only, so the screen's choices are made on
   the next client tick.
2. An options page id (`options.key_binds`) for a button on the right page, and a way to open a mod screen
   from a screen hook (as for Controlling).

## To check against the NeoForge original (on a machine with the jar)

1. Whether purging unbinds, hides, or both, whether Keep restores the key, and whether a purge is undone on uninstall.
2. Its config's name, format and defaults (any built-in list of bindings it purges by default).
3. Which binding of a conflicting pair it keeps.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
