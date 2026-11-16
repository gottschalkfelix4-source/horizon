# No Villager Death Messages (Horizon port)

Port of [No Villager Death Messages](https://www.curseforge.com/minecraft/mc-mods/no-villager-death-messages)
6.0.0 (ATM10 8.x, MIT, by Serilum) as a native Horizon mod. Written from the mod's public
description, not from its code.

## What it does

| Behaviour | Status |
|---|---|
| No server-log line `Villager ... died, message: '...'` when a villager dies | holds without code: Horizon's server never writes that line (vanilla's `Villager.die` log is not ported) |

The mod is server-only and has no config. This port registers no hook; it exists so a modpack
listing it resolves and so the suppression has a home if Horizon ever ports the log line.

## Needs from Horizon

- Nothing today. If the engine ports vanilla's `Villager.die` log line, the port needs a way to
  stop it, for example a cancellable `villager_death_log(host, event: { entity: u32, message:
  &str, cancelled: bool })` hook, or `console_line` with `cancel` (today `console_line` is
  listen-only and is heard a tick later, so it cannot stop a line).

## To check against the NeoForge original (oracle on a machine with the jar)

1. That the mod only touches the log line, and not a chat message (a named villager's death
   message, or a tamed animal's to its owner, which vanilla sends through `TamableAnimal.die`).
2. Whether it also covers wandering traders or other `AbstractVillager`s.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
