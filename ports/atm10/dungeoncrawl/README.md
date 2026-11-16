# Dungeon Crawl (Horizon port)

Port of Dungeon Crawl 2.3.17 (ATM10 8.x, GPL-3.0-only, by xiroc) as a native Horizon mod.
Written from the mod's observed behaviour and public description, not from its code. Every
number below is from memory of how the mod plays and must be checked against the original.

## What it does

| Feature | Status |
|---|---|
| Underground multi-floor dungeons: a structure type `dungeoncrawl:dungeon` with a data pack structure and structure set | ported (a first slice), not yet checked against the original |
| Layout: 4 floors of 7x7 cells (9 blocks each), joined by a random spanning tree, so every cell is reachable | ported |
| Rooms at dead ends and on about one cell in four, the rest 3-wide corridors | ported |
| Ladder shaft from the surface to the first floor, and between floors | ported (the original uses staircases) |
| Spawners in rooms, harder mobs deeper (zombie, skeleton, spider, cave spider) | partly: one vanilla mob per floor, no spawner tweaks |
| Loot chests in rooms | partly: vanilla's `minecraft:chests/simple_dungeon` table, not the mod's own tables |
| Themes (stone bricks, desert, jungle, nether, ...) by biome and depth; secondary themes | not yet: stone bricks with mossy and cracked wear only |
| Room variety (large rooms, node rooms, secret rooms, fountains, libraries), bridges and trap corridors | not yet |
| The entrance building at the surface | not yet: a stone-brick ladder shaft |
| Config (floors, spacing, mob and loot overrides) | not yet: fixed to the values below |

How: `find_generation_point` draws the entrance cell, each floor's layout (`floor_layout`) and
its way down from the chunk's structure random, and answers one `Entrance` piece and one `Floor`
piece per floor. A floor saves its 49 cells' flags with the chunk (`cells`), so a piece loaded
later builds the same blocks. `post_process` places only what `placed` says: open space (carved
to air), the walls, floor and ceiling bordering it, ladders, spawners (block entity
`SpawnData`) and chests (block entity `LootTable` with a seed from the piece random); the rock
between rooms is left as it is.

## To check against the NeoForge original (oracle on a machine with the jar)

1. Floors per dungeon (4 here), grid and cell size (7x7 cells of 9 blocks), storey height (7).
2. Depth: here the first floor's ceiling is 8 blocks under the surface at the entrance, and a
   dungeon needs dry land (surface above sea level) and room above the world's bottom.
3. Spacing and separation of the structure set (32 and 12 here) and its salt.
4. The biomes it generates in (`tags/worldgen/biome/has_structure/dungeon.json` here).
5. The mobs per floor and the loot tables per floor.
6. How floors connect (staircases in the original, ladders here) and where the entrance stands.

## Needs from Horizon

- Nothing for this slice: the structure type, its pieces and block entity NBT are in the API.
  Themes need a biome read in `PieceContext` (`fn biome(&mut self, pos: [i32; 3]) -> BiomeId`,
  beside `GenerationContext::noise_biome` in `crates/horizon-mod-vocab/src/world/structure_type.rs`),
  to pick a theme per piece where the original does.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
