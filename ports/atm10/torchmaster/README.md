# Torchmaster (Horizon port)

Port of [Torchmaster](https://github.com/xalcon/TorchMaster) 21.1.13 (ATM10 8.x, MIT) as a native
Horizon mod. Written from the mod's observed behaviour and public description, not from its code.

## What it does

| Block | Effect | Status |
|---|---|---|
| `torchmaster:megatorch` | Refuses natural spawns of `MobCategory::Monster` within 64 blocks on each axis (a cube), and a village siege's zombies | ported, not yet checked against the original |
| `torchmaster:dreadlamp` | Refuses natural spawns of `MobCategory::Creature` within 64 blocks on each axis | ported, not yet checked against the original |
| `torchmaster:feral_flare_lantern` | Places invisible light blocks around it | not yet |
| Config (radius, entity block/allow lists, which spawn reasons) | | not yet: fixed to the defaults below |

How: placing one of the blocks from its item records it on its chunk (attachment
`torchmaster:lights`, saved with the chunk). The spawn placement check reads the records of the
level's loaded chunks and refuses the spawn when a matching block is still at a recorded
position and the spawn lies in its cube. Spawners, spawn eggs, `/summon`, breeding and
chunk generation are left alone.

## Needs from Horizon

- `Host::entity_type_category(kind)`: the category of a mob type before the mob exists. Added
  in this fork (`crates/horizon-mod-vocab/src/host/entities.rs`,
  `crates/horizon-client/src/game/modding/native_entity_data.rs`); to send upstream as a PR.

## To check against the NeoForge original (oracle on a machine with the jar)

1. The area's shape: cube or sphere, and whether 64 is inclusive.
2. Which spawn reasons are refused by default (natural only, plus sieges? patrols? chunk
   generation?).
3. A block placed by something other than a player's item (a mod machine, `/setblock`, a
   structure) is not recorded here. Does the original protect from those?
4. The blocks' facts (hardness, light, shape, drops, recipe) come from `horizon-port-pack carry`
   on the jar; `data/torchmaster/horizon/block/*.json` here are placeholders for building without it.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
