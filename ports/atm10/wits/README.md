# WITS (Horizon port)

Port of [WITS](https://modrinth.com/mod/wits) ("What Is This Structure") 1.3.1 (ATM10, MIT) as a
native Horizon mod. Server-only: vanilla clients can use it. Written from the mod's public
description and observed behaviour, not from its code.

## What it does

| Command | Effect | Status |
|---|---|---|
| `/wits` (everyone) | Lists the structures at the player's block | ported for vanilla structures, not yet checked against the original |
| `/witsop [dimension] [pos]` (operators, level 2) | The same for any position in any dimension | ported for vanilla structures, not yet checked |
| Structures added by other mods or data packs | | not yet: needs the read below |

Features (dungeons, desert wells) are not structures and are never found, as in the original.

How: the mod API has no "structures at a position" read, so each lookup asks the server's
command runner `execute in <dim> positioned <pos> if predicate wits:minecraft/<structure>` once
per vanilla structure. The 34 predicates (`data/wits/predicate/minecraft/*.json`) are vanilla
`location_check` predicates with `structures`, which test whether the block is inside one of
the structure's pieces. Structures a data pack removed from the registry are skipped.

## Needs from Horizon

- `LevelRef::structures_at(self, host: &dyn Host, pos: [i32; 3]) -> Option<Vec<String>>`
  (`StructureManager.getAllStructuresAt` / `getStructureWithPieceAt` for every structure): every
  structure, modded ones included, without one predicate file per structure.
- The workaround depends on Horizon loading a server mod's `data/<id>/predicate/` files as a data
  pack's and on `execute if predicate` resolving them; to confirm in game.

## To check against the NeoForge original (oracle on a machine with the jar)

1. Whether the original tests the structures' bounding boxes or their pieces (pieces here), and
   whether it lists the structure's tag-based names or registry ids (registry ids here).
2. The exact feedback lines and the permission level of `/witsop` (2 assumed).
3. The author's name in LICENSE-ORIGINAL.md (from memory; confirm on the project page).

## Build

```sh
CARGO_TARGET_DIR=/home/user/atm10-target cargo test      # in this folder
horizon-launcher native-build . --instance <instance> --trust-native-code
```
