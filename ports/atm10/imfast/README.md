# I'm Fast (Horizon port)

Port of [I'm Fast](https://modrinth.com/mod/im-fast) 1.0.3 (ATM10, MIT) as a native Horizon mod.
Server-only. Written from the mod's public description, not from its code.

## What it does

| Feature | Status |
|---|---|
| No "moved too quickly" correction for players walking, flying or riding nothing | ported through the `player_movement_check` game rule |
| No "moved too quickly" correction while gliding with elytra | ported through the `elytra_movement_check` game rule |
| No "moved wrongly" correction | not yet: needs the hook below |
| No "vehicle moved too quickly" / "vehicle moved wrongly" correction | not yet: needs the hook below |

How: at server start the mod records the world's two rules in
`<world>/horizon-mod-data/imfast/saved_rules.json` and sets both to `false`; at server stop it puts
the recorded values back and clears the record. A record left by a crash is restored from at
the next start, so uninstalling the mod never leaves the checks off. The original changes no
game rule: while the server runs, `/gamerule` shows `false` here, and a value an operator sets
meanwhile is replaced by the recorded one at stop.

## Needs from Horizon

- A server hook over the movement checks, for players and vehicles, e.g.
  `fn player_movement_checks(&self, host: &dyn Host, event: &mut PlayerMovementChecksEvent<'_>)`
  with `player: PlayerRef`, `vehicle: Option<EntityRef>`, `check_too_quickly: &mut bool`,
  `check_wrongly: &mut bool` (`ServerGamePacketListenerImpl.handleMovePlayer` /
  `handleMoveVehicle`). With it the mod would leave the game rules alone.

## To check against the NeoForge original (oracle on a machine with the jar)

1. Whether the original removes the checks entirely or raises their limits.
2. Whether it covers vehicles and "moved wrongly" in 1.0.3, and whether it has a config.

## Build

```sh
CARGO_TARGET_DIR=/home/user/atm10-target cargo test      # in this folder
horizon-launcher native-build . --instance <instance> --trust-native-code
```
