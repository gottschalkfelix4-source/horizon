# More Overlays (Horizon port)

Port of More Overlays Updated 1.24.2 (ATM10 8.x, MIT) as a native Horizon client mod. Written
from the mod's observed behaviour and public description, not from its code.

## What it does

| Feature | Status |
|---|---|
| Light level overlay, F7: a red cross on blocks where monsters can spawn at any time, a yellow one where they can spawn only at night | ported; draws nothing yet on a client that is not also the server (see "Needs from Horizon") |
| Chunk boundaries, F9: cycles Off → corners of the player's chunk and its neighbours → plus a 2-block grid on the chunk's walls → plus the region's corners | ported, colours and modes not yet checked against the original |
| Inventory search, Ctrl+F on any container screen: a search field under the panel, slots whose item name doesn't contain every typed word are darkened | ported (item names only) |
| JEI search highlight in inventories | not yet: needs JEI ported first |
| Search by tooltip, mod name (`@mod`) or tag | not yet |
| Config (overlay range, colours, chunk modes, keep search between screens) | not yet: fixed to the values in `src/lib.rs` |

All three keys are listed under "More Overlays" in Options > Controls and can be rebound.
Strings are translation keys; the English is in `assets/moreoverlays/lang/en_us.json`.

How the light overlay decides (`src/light.rs`): a space is marked when the block below has a full
top face (its outline) and isn't one vanilla never spawns monsters on (glass, leaves, ice,
bedrock, barrier, magma, blocks giving light 14 or 15), and the space itself is not a full
collision block, holds no fluid and isn't in `minecraft:prevent_mob_spawning_inside` (rails).
Then, with the dimension type's `monster_spawn_block_light_limit` and the top of its
`monster_spawn_light_level`: block light over the limit marks nothing; brightness by day within
the level is red; brightness at night (sky light minus 11) within it is yellow. The area is
16 blocks around the player and 8 up and down, rebuilt every 10 ticks.

## Needs from Horizon

1. **Light levels on a client.** `LevelRef::brightness(host, LightLayer, pos)`
   (`level_brightness` in `crates/horizon-mod-vocab/src/host/levels.rs`) is `Side: server`:
   the game answers it from `native_level_world`, which refuses a client, so on a window joined
   to its server thread (every single-player world) it returns `None` and the overlay draws
   nothing. Proposal: make it `Side: both` and answer on a client from the client's world
   (`native_level_read_world`, as `level_block_state` does; vanilla `ClientLevel.getBrightness`).
   No signature change.
2. **Spawn rules of a block state.** The client has no read of `BlockState.isValidSpawn` or
   `NaturalSpawner.isValidEmptySpawnBlock`, so the port keeps its own list of blocks that never
   take a spawn and ignores redstone signal sources and blocks that hurt mobs. Horizon already
   exports both as flags (`horizon-client/src/mobs/spawn_rules.rs`). Proposal, in
   `host/block_states.rs` (`BlockStatesHost`, `Side: both`, `#[access(Metadata)]`):
   `fn block_state_spawn_rules(&self, state: BlockState, entity: EntityTypeKey) -> Option<SpawnRules>`
   with `SpawnRules { valid_spawn: bool, valid_empty_spawn: bool }`.

## To check against the NeoForge original (oracle on a machine with the jar)

1. The overlay's range (horizontal and vertical) and refresh rate; whether it marks the space
   or the block below, and its cross colours and height.
2. Whether a space under a 1-block ceiling (no room for a zombie) is marked.
3. The chunk-boundary modes, their order, colours and the grid spacing.
4. Where the search field sits, whether the search survives closing the screen, and whether
   matching slots are outlined as well as the others darkened.
5. Whether the search key is Ctrl+F or a bare key with a click on the field.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
