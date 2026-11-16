# Silent's Gems (Horizon port)

Port of [Silent's Gems](https://github.com/SilentChaos512/SilentGems) 5.1.3 (ATM10, MIT) as a
native Horizon mod. Written from the mod's public description and observed behaviour, not from
its code. This first pass covers the gem content and the teleporters; the rest is listed below.

## What it does

| Feature | Status |
|---|---|
| 48 gems in three sets of 16 (Overworld, Nether, End), declared in code (`blocks`, `items` hooks, `src/gems.rs`) | ported, names to check |
| Per gem: ore (plus a deepslate ore for the Overworld set), storage block, bricks, glass, glowrose (light 10), teleporter | declared from vanilla templates |
| Per gem: the gem item and a Return Home Charm; one Teleporter Linker | declared |
| Teleporter Linker: use on a teleporter to bind it, on a second to link both ways (one way when they are in different dimensions) | ported |
| Teleporter: use with an empty hand to go to its linked teleporter (any dimension), arriving on top of it; a missing destination is reported | ported |
| Return Home Charm: use on a teleporter to bind it; hold use for 3 s to warp there | ported, time assumed |
| Ore generation (configured and placed features, biome modifiers), recipes, loot tables, tags, models and textures | data carried from the jar (`horizon-port-pack carry`), not written here |
| Teleport cost and the "same dimension only" config, redstone teleporters, anchor teleporters | not yet |
| Soul gems, chaos, gem lamps, fluffy puffs, gem tools/armour (now Silent Gear), enchantment tokens, gear souls | not yet |

## Needs from Horizon

- Linking a teleporter in another dimension needs a server write to another level's chunk
  attachment: today an attachment write goes to the host's current level
  (`Holder::Chunk([x, z])`). Wanted: `LevelRef::attachments(self, host: &mut dyn ServerHost)` or a
  `Holder::ChunkIn(LevelKey, [i32; 2])`. Until then a cross-dimension link is one way (the
  second teleporter sends to the first; the first stays as it was).
- A teleporter destroyed keeps its record until it is relinked (harmless: `warp` checks the
  block is still a teleporter), as Torchmaster's port does.

## To check against the NeoForge original (oracle on a machine with the jar)

1. The gem list and ids (`src/gems.rs`): names, which set each belongs to, and which block kinds
   each gem has in 5.1.3 (shards? lamps? deepslate and nether ores?).
2. The templates: hardness, sounds, harvest level and experience of ores; glowrose light level.
3. Teleporter use: right click with an empty hand, or standing on it? Cost (chaos, XP, none),
   cross-dimension allowed by default, arrival spot.
4. Linker: whether linking is always both ways and what it says; whether a linker keeps its
   binding after linking.
5. Charm: hold time, whether it needs a linked teleporter or just a bound one, cooldown, durability.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
