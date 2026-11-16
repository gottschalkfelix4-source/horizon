# Crash Utilities (Horizon port)

Port of [Crash Utilities](https://www.curseforge.com/minecraft/mc-mods/crash-utilities) 9.0.4
(ATM10, MIT, by Darkere) as a native Horizon mod. Server-only: players need nothing installed.
Written from the mod's public description and its observed behaviour, not from its code.

## What it does

Every command needs operator level 2. Feedback is translatable
(`assets/crashutilities/lang/en_us.json`) and carries its English line as the fallback, so vanilla
clients read it.

| Feature | Command / setting | Status |
|---|---|---|
| Entity counts per type, per level | `/cu entities list [dimension]` | ported, not yet checked against the original |
| Chunks holding the most of one entity type (click a line to get a `tp` command) | `/cu entities find <type> [dimension]` | ported, not yet checked |
| Remove every loaded entity of one type | `/cu entities remove <type> [dimension]` | ported, not yet checked |
| Loaded chunks per level | `/cu loadedchunks [dimension]` | ported (counts only) |
| Send a player to the world spawn | `/cu unstuck <player>` | ported, online players only |
| Move a player into another dimension | `/cu teleport <player> <dimension> <pos>` | ported, online players only |
| Read and empty slots of a player's inventory | `/cu inventory read <player>`, `/cu inventory remove <player> <slot>` | ported, online players' main inventory only |
| Memory in use | `/cu memory` | ported as the process's resident memory (Linux); the original reports JVM heap and keeps a history |
| Clear ground items above a limit | `config/crashutilities.json` `item_clear` | ported, off by default |
| Block entity counts and finding them | `/cu tileentities ...` | not yet: needs block-entity listing (below) |
| Map views of loaded chunks / entity counts | | not yet: needs a client map screen; no client part in this server-only port |
| Open another player's inventory as a GUI | | not yet: needs a menu backed by another player's inventory (below) |
| Offline players' inventories, Curios slots | | not yet (below) |
| Fetch log files and crash reports | | not yet: the original uploads them; Horizon's built-in crash report covers part of it |

`config/crashutilities.json` (all keys optional):

```json
{ "item_clear": { "enabled": false, "maximum": 1000, "interval_ticks": 1200 } }
```

## Needs from Horizon

- Block entities of a loaded chunk: `ChunkRef::block_entities(self, host: &dyn Host) -> Option<Vec<BlockEntityRef>>`
  (or `Host::level_block_entities(level: LevelRef) -> Option<Vec<([i32; 3], BlockEntityTypeKey)>>`) for `/cu tileentities list|find`.
- Offline player data: `ServerHost::offline_player_inventory(uuid: u128) -> Option<Vec<(u32, ItemStack)>>`
  and `offline_player_set_inventory_slot(uuid, slot, Option<ItemStack>) -> Result<(), PlayerDataRefusal>`.
- A chest-style menu whose slots are another player's inventory:
  `PlayerRef::open_inventory_of(self, host: &mut dyn ServerHost, target: PlayerRef) -> bool`.
- Curios: no Curios API in Horizon yet.

## To check against the NeoForge original (oracle on a machine with the jar)

1. Command names and argument order (`/cu teleport` here takes a required position; the original may
   default to the target dimension's spawn). Required permission level (2 assumed).
2. Whether `entities remove` skips named or tamed entities; here it removes every loaded entity of the type.
3. The item-clear config keys, defaults (1000 items, every 60 s assumed), its warning messages, and
   whether it counts per level or across all levels (all levels here).
4. `unstuck`: the original may use the player's bed/respawn point before the world spawn.

## Build

```sh
CARGO_TARGET_DIR=/home/user/atm10-target cargo test      # in this folder
horizon-launcher native-build . --instance <instance> --trust-native-code
```
