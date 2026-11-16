# Torchmaster (Horizon port)

Port of [Torchmaster](https://github.com/xalcon/TorchMaster) 21.1.13 (ATM10 8.x, MIT) as a native
Horizon mod. Written from the mod's observed behaviour and public description, not from its code.

## What it does

| Block | Effect | Status |
|---|---|---|
| `torchmaster:megatorch` | Refuses natural, patrol and chunk-generation spawns of `MobCategory::Monster` in a 64-block cube (one block more upwards), as observed on the original | ported, not yet checked against the original |
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

## Measured on 2026-11-16 (Mac, NeoForge 21.1.259 original vs this port in game)

- Mega Torch in game: 0 of 288 new hostile mobs inside the 64-block cube; without it 25 of 281 (9 in the cube's corners, so it is a cube, not a sphere).
- Original, by firing NeoForge's spawn checks at fixed offsets: a cube; 60/60/0, 64/0/64, 64/64/64 blocked, 65/0/0 and 65/0/65 not. One block more upwards. Blocks NATURAL, PATROL and CHUNK_GENERATION, not EVENT. This port now does the same.
- Still different: the original also protects a torch set with `/setblock`; this port records only torches placed from the item. The Dread Lamp is untested.
- Engine gaps: the game refuses `side = "server"` for a mod with block data (now `both`); `horizon-port-pack carry` drops both blocks ("no vanilla Block with properties [waterlogged]"), so the `data/` files here are placeholders; real values: hardness 1, light 15, a narrow column shape.
- A game built straight from a checkout does not load native mods; the test used a local crate that links the port in. Mod loading goes through the launcher and an SDK build.
