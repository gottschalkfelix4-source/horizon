# MineColonies (Horizon port)

Port of [MineColonies](https://github.com/ldtteam/minecolonies) 1.1.1403-1.21.1 (ATM10 8.x,
GPL-3.0-or-later) as a native Horizon mod, under the same licence. Written from the mod's observed
behaviour and public description (its wiki), not from its code. MineColonies is very large; this
is its first slice: the colony itself, its land and its protection. Every number below is from
memory and has to be checked against the original (see the list at the end).

## What it does

Placing a Town Hall (`minecolonies:blockhuttownhall`) founds a colony owned by the player who
placed it. The colony claims a square of chunks around the Town Hall's chunk (4 on each side).
In those chunks only the owner and the colony's officers may place or break blocks; anyone else
is refused with a message naming the colony. A Town Hall is refused when its player already owns
a colony in this world, or when its claim would come closer than 8 chunks to another colony's.

```text
/minecolonies colony addOfficer <colony id> <player>
/minecolonies colony removeOfficer <colony id> <player>
/minecolonies colony delete <colony id>
```

The colony's owner and operators (level 2) may run them.

| Feature | Status |
|---|---|
| Founding a colony with a Town Hall, one per player | ported (per world, see below) |
| Chunk claims around the Town Hall, distance between colonies | ported |
| Protection: only owner and officers place and break blocks in the claim | ported |
| Officers, deleting a colony (commands) | ported (as commands; the original also has the Town Hall screen's permission page) |
| Supply Camp and Supply Ship (the items that place the first buildings) | not yet |
| Citizens (entities, needs, happiness, skills), their AI and jobs | not yet |
| Hut blocks and the Build Tool, schematics (Structurize), the Builder placing buildings | not yet |
| Building levels, claim growth with buildings, the Town Hall's research tree | not yet |
| Raids (barbarians, pirates...), guards, colony events | not yet |
| Requests and the warehouse/courier delivery system | not yet |
| Permission ranks beyond owner and officer, interaction rules (doors, containers, attacks) | not yet |
| Colony list and teleport commands, colony map | not yet |

How: a world's colonies are kept in the level attachment `minecolonies:colonies` (saved with the
level): id, owner, officers, Town Hall position and claim radius. `entity_place` refuses a
foreign placement in a claim and a Town Hall that would found nothing; the Town Hall item's
`on_block_placed_by` founds the colony; `break_block` refuses a foreign break in a claim.

## To check against the NeoForge original

1. The claim's size (`initialColonySize`, here 4 chunks each side) and shape (square here), and
   how it grows with buildings.
2. The distance between colonies (`minColonyDistance`, here 8 chunks between claims), and whether
   it is counted between claims or between Town Halls.
3. One colony per player: here per level (dimension); the original counts per server.
4. Who is protected against: the original has ranks (owner, officer, friend, neutral, hostile)
   with a permission table per action; here only owner and officers build, everyone else is
   refused, operators included.
5. Breaking the Town Hall: here the colony stays (its owner may delete it); the original asks for
   confirmation and may handle it differently.
6. The commands' exact names and arguments.
7. The Town Hall's facts (hardness, model, recipe) come from `horizon-port-pack carry` on the jar;
   `data/minecolonies/horizon/block/blockhuttownhall.json` is a placeholder for building without
   it.

## Needs from Horizon

- World-wide mod data, for colonies that span dimensions and a one-colony-per-player rule across
  them (the level attachment is per dimension, and which level `Holder::Level` means outside a
  level's own hooks is not stated). Proposed: `Holder::World` in
  `crates/horizon-mod-runtime/src/attachments.rs`, saved in the world's `data/` folder, reached
  from every server hook.
- The player in `EntityPlaceEvent` as a `PlayerRef`, so a refusal can be told to the placer
  without going through `host.players().acting_player()` and comparing entities:
  `pub placer_player: Option<PlayerRef>` on `EntityPlaceEvent` (`crates/horizon-mod-vocab/src/hooks/entity_place.rs`).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
