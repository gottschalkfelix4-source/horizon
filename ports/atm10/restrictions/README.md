# Restrictions (Horizon port)

Port of [Restrictions](https://www.curseforge.com/minecraft/mc-mods/restrictions) 1.21-7.0.3
(ATM10, MIT, by McJty) as a native Horizon mod. Both sides (it adds blocks). Written from the
mod's public description and observed behaviour, not from its code.

## What it does

| Block | Effect | Status |
|---|---|---|
| `restrictions:attractor` | Pulls entities in the one-block-wide beam in front of it towards it; the beam is as long as the redstone signal it receives (off without one) | ported, not yet checked against the original |
| `restrictions:pusher` | The same, pushing away | ported, not yet checked |
| `restrictions:one_way` | See-through, no collision; anything inside moves along its facing at 0.4 blocks/tick, no redstone needed (facing up: an elevator) | ported, not yet checked |
| Models, textures, recipes, loot tables | | come from `horizon-port-pack carry` on the jar; not in this folder |

How: the blocks carry a six-way `facing` (declared in `data/restrictions/horizon/block/*.json`,
placeholders on vanilla templates so the port builds without the jar). The Attractor and Pusher
face the player who places them (as a piston), the One Way faces where the player looks.
Placing an Attractor or Pusher from its item records it on its chunk (attachment
`restrictions:devices`); after each level tick every recorded device still standing and powered
adds 0.08 blocks/tick to the speed of the entities in its beam, until they move 0.6 blocks/tick
along it. A record whose block is gone is ignored. The One Way answers `entity_inside` by setting
the entity's movement and resetting its fall distance.

## Needs from Horizon

- A device placed by something other than a player's item (`/setblock`, a structure, a mod
  machine) is not recorded, so it does nothing. A block hook on placement of the state
  (`Block.onPlace`, e.g. `fn on_place(&self, level: &mut dyn LevelWriteHost, event: &mut OnPlaceEvent)`
  with `pos`, `state`, `old_state`) or a ticking block entity type declared in data would cover it.
- Moving players: the push goes through `EntityRef::push`; whether the player's client is sent
  the new motion (vanilla `hurtMarked`) must be checked in game.

## To check against the NeoForge original (oracle on a machine with the jar)

1. The beam's width (one block here) and whether range equals the signal strength exactly.
2. The pull/push speeds and the One Way's speed (0.08 per tick, cap 0.6, and 0.4 assumed).
3. Which way each block faces when placed; whether the One Way keeps an entity's sideways motion.
4. Whether players are affected, and whether items and projectiles are.

## Build

```sh
CARGO_TARGET_DIR=/home/user/atm10-target cargo test      # in this folder
horizon-launcher native-build . --instance <instance> --trust-native-code
```
