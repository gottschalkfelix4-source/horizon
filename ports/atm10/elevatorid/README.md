# ElevatorMod (Horizon port)

Port of [ElevatorMod](https://github.com/VsnGamer/ElevatorMod) 1.21.1-1.11.4 (ATM10, MIT) as a
native Horizon mod. Written from the mod's observed behaviour and public description, not from its
code.

## What it does

| Feature | Behaviour | Status |
|---|---|---|
| 16 elevator blocks `elevatorid:elevator_<colour>` | Placeable blocks, one per dye colour | ported (placeholder block facts) |
| Ride up | Jumping while standing on an elevator teleports the player onto the nearest elevator above in the same column, within 384 blocks, that has room for a player on top | ported, not yet checked against the original |
| Ride down | Starting to sneak on an elevator does the same downwards | ported, not yet checked against the original |
| Arrival sound | A sound plays to the rider | ported with a vanilla sound (`entity.enderman.teleport`) in place of the original's own |
| Same-colour only | Off by default; the logic supports it (`SAME_COLOR`) | ported as a constant, no config file yet |
| Camouflage (right-click an elevator with a block to disguise it) | | not yet |
| Directional mode and its settings screen (face the player on arrival, hide the arrow) | | not yet |
| XP cost per ride, mobs riding | | not yet (both off by default in the original) |
| Recipes (wool + ender pearl), dyeing an elevator | | not yet: come from `horizon-port-pack carry` on the jar |

How: `player_jump` and `player_toggle_sneak` (sneaking = true) look at the block the player
stands on (`Entity.getOnPos`). If it is an elevator, the column is searched block by block
upwards or downwards; an elevator whose top has no room for a player (block collision in a
0.6 x 1.8 box above it) is passed over. The player is teleported to the centre of the
destination's top face.

## Needs from Horizon

- Camouflage: a block entity holding the disguise state is available (`BlockEntityPart`), but
  drawing the elevator with another block's model needs a per-position model override on the
  client, e.g. `ClientHost::set_block_model_override(level: LevelRef, pos: [i32; 3], state:
  Option<BlockState>)`, or a block-entity-driven baked model hook. Not found in the API.
- Config: the original's `range`, `sameColor`, `precisionTarget`, `useXP`, `XPPointsAmount`,
  `mobsCanUseElevator` need a config file; the API's config area was not wired up in this port.

## To check against the NeoForge original (oracle on a machine with the jar)

1. Whether an obstructed elevator is passed over (as here) or ends the search.
2. The room needed on top: player height (1.8) as here, or two non-solid blocks.
3. Whether the rider keeps their x/z offset and look direction (`precisionTarget`), and the
   y they arrive at.
4. The arrival sound's id and volume.
5. Whether sneaking must start while on the ground, and whether holding sneak repeats.
6. Block facts (hardness, sound, tool) and recipes from `horizon-port-pack carry`;
   `data/elevatorid/horizon/block/*.json` are placeholders on wool templates.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
