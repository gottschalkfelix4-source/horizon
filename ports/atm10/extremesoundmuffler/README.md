# Extreme Sound Muffler (Horizon port)

Port of [Extreme Sound Muffler](https://github.com/LeoBeliik/ExtremeSoundMuffler) 3.57 (ATM10 8.6,
LGPL-3.0-only) as a native Horizon client mod. Written from the mod's observed behaviour and public
description, not from its code. **Licence: LGPL-3.0-only**, as the original (`license` in `Cargo.toml`,
`LICENSE-ORIGINAL.md`).

## What it does

Every sound the client is about to play passes through the `play_sound` hook: it is noted in the list
of recent sounds, and its volume is multiplied by the player's choice for it (a factor of 0 does not
play it at all). The screen opens from its key (`Open Sound Muffler`, unbound by default) or the note
button over the inventory's top right corner.

| Feature | Status |
|---|---|
| List of recently heard sounds, of every sound event, of muffled sounds; search box | ported |
| Click a sound to muffle it (at the "Muffle volume" setting, 0 % by default) or unmuffle it | ported |
| `-` / `+`: a muffled sound's volume in 10 % steps, 0 % to 90 % | ported |
| Muffle everywhere (global) | ported |
| Ten anchors: set at the player's position, radius 1 to 128 (default 32, steps of 4), their own muffled sounds; overlapping anchors take the quietest | ported |
| On/off switch for all muffling | ported |
| Button on the inventory screen (setting to hide it) | ported |
| Choices kept across sessions (`horizon-mod-data/extremesoundmuffler/muffler.txt`) | ported |
| Anchors kept per world or server | not yet: anchors are kept with their dimension, for every world |
| Anchor names, anchor list on the map / in the world | not yet |
| Play a sound from the list to preview it | not yet (`ClientHost::play_local_sound` can do it) |

Code: `src/muffle.rs` (factors, anchors, recent list, saved form; unit tests), `src/lib.rs` (hooks, screen).
Texts: `assets/extremesoundmuffler/lang/en_us.json`. Settings (Options > Mods): `Muffle volume`,
`Button over the inventory`.

## Needs from Horizon

1. **Which world the client is in**, to keep anchors per world as the original does: in
   `crates/horizon-mod-vocab/src/host/client_view.rs`, `fn world_key(&self) -> Option<String>`, the save's
   folder name in single player and the server's address in multiplayer (no player name, no IP for LAN).
2. **The client host in `play_sound`.** It gets only a `LoopContext`, so the dimension is copied from the
   last client tick, and the state is shared through a lock that a sound played while the screen holds it
   skips (`try_lock`). A read-only `&dyn ClientHost` there (or `dimension` in `PlaySoundEvent`) would remove
   both.

The `muffle_sounds` hook is not used: it is a short, fading muffle of every sound (ears ringing after a
blast), not a per-sound one.

## To check against the NeoForge original (on a machine with the jar)

1. The volume slider's range and steps, and the default muffle volume.
2. Anchor count, default and maximum radius, shape (sphere or cube), and how overlapping anchors and the
   global list combine.
3. Whether relative sounds (music, UI clicks) can be muffled and by anchors.
4. The inventory button's place and the screen's layout and keys.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
