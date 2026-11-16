# Simple Backups (Horizon port)

Port of [Simple Backups](https://modrinth.com/mod/simple-backups) 1.21-4.0.30 (ATM10,
Apache-2.0) as a native Horizon mod. Server-only. Written from the mod's public description and
observed behaviour, not from its code.

## What it does

| Feature | Status |
|---|---|
| Back up the world as a zip every `timer` minutes (120) into `<game folder>/simplebackups/` | ported, not yet checked against the original |
| Only when a player was online since the last backup, unless `no_player_backups` | ported |
| Keep `backups_to_keep` (10) backups and at most `max_disk_size` ("25 GB"); oldest deleted first | ported |
| Backup types `FULL_BACKUPS`, `MODIFIED_SINCE_LAST`, `MODIFIED_SINCE_FULL` (a full one at least every `full_backup_timer` minutes) | ported (partial backups hold the changed files only) |
| `compression_level`, `send_messages`, `output_path`, `enabled` | ported |
| `/backup start [quiet]`, `/backup pause`, `/backup unpause` (operators, level 2) | ported |
| `/backup mergeBackups` (merge partial backups into a full one) | not yet |
| Zipping off the server thread | ported: `save-all flush` and `save-off` before, `save-on` when the worker ends |

Settings live in `config/simplebackups.json` (all keys optional; the original uses a TOML file):

```json
{ "enabled": true, "backup_type": "FULL_BACKUPS", "timer": 120, "full_backup_timer": 1440,
  "backups_to_keep": 10, "max_disk_size": "25 GB", "compression_level": -1,
  "send_messages": true, "no_player_backups": false, "output_path": "simplebackups" }
```

The timer and the pause survive restarts in `<world>/horizon-mod-data/simplebackups/state.json`.

## Needs from Horizon

Nothing for the ported part. Two reads would make it more exact:

- `Commands::run_command` for `save-off`/`save-on` is the console's route; a host method
  `ServerHost::set_autosave(&mut self, on: bool)` would avoid printing to the console.
- A "world saved" signal after `save-all flush` (the `level_save` hook exists; the port assumes the
  command flushes synchronously).

## To check against the NeoForge original (oracle on a machine with the jar)

1. Config key names, defaults (assumed above) and the TOML layout; the backup file name pattern.
2. Whether "no players" means none online now or none since the last backup (the latter here).
3. Whether messages go to everyone or to operators only (everyone here).
4. The zip's inner layout (a top folder named after the world here).

## Build

```sh
CARGO_TARGET_DIR=/home/user/atm10-target cargo test      # in this folder
horizon-launcher native-build . --instance <instance> --trust-native-code
```
