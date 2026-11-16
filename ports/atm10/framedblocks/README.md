# FramedBlocks (Horizon port)

Port of [FramedBlocks](https://github.com/XFactHD/FramedBlocks) 10.6.3 (ATM10 8.x,
LGPL-3.0-only) as a native Horizon mod. Written from the mod's observed behaviour and public
description, not from its code. Every rule below is from memory of how the mod plays and must
be checked against the original.

## What it does

| Feature | Status |
|---|---|
| Framed Cube camouflage: right-click with a block puts it in the frame (one item used, not in creative); a second block is placed against the frame as usual | ported (server side), not yet checked against the original |
| Glowstone dust makes a frame glow (light 15, its `glowing` state) | ported |
| Framed Hammer takes the camouflage and the glowstone back out, into the player's inventory | ported |
| Breaking a frame drops its camouflage and glowstone (not in creative) | ported for breaking by a player; explosions and other removals lose it |
| Drawing the camouflage (the frame looks like the block it holds) | not yet: needs a client model hook (below) |
| The other ~150 framed shapes (slabs, stairs, walls, doors, slopes, panels, pillars...) | not yet |
| Double camouflage (slabs, panels), intangibility (phantom paste), reinforcement, emissive camo | not yet |
| Framing Saw, Blueprint, Framed Key/Wrench and the other tools | not yet |
| Camouflage from fluids, from a block entity's item, connected textures | not yet |

How: the cube's `BlockPart::use_item_on` reads the held stack (`click` in `src/lib.rs` decides,
pure and tested), writes the camouflage to the chunk attachment `framedblocks:camo` (saved
with the chunk, in the level in place, which is the clicking player's) and resends the block, and flips the `glowing` state for glowstone.
`BlockPart::on_destroyed_by_player` drops the contents.

## To check against the NeoForge original (oracle on a machine with the jar)

1. Which blocks may camouflage a frame. Here: any block with an item, no block entity and not
   replaceable. The original also refuses non-full or non-solid shapes and has a blacklist tag.
2. Whether a camouflaged frame gives the camouflage back on a sneak-click with an empty hand
   (here only the hammer does).
3. Whether glowstone is a state (`glowing` here) or block entity data, and its light level.
4. Whether breaking the frame in survival drops the camouflage (here: yes).
5. The camouflage's effect on the frame's sound, hardness, light, friction and redstone
   (here: none).
6. Block and item facts (`data/framedblocks/horizon/**`) are placeholders until
   `horizon-port-pack carry` runs on the jar.

## Needs from Horizon

- A client model hook keyed by a block's position, so a frame draws its camouflage's model:
  `fn block_model_override(&self, host: &dyn ClientHost, event: &mut BlockModelOverrideEvent) -> Option<BlockState>`
  on `BlockPart` (`crates/horizon-mod-vocab/src/hooks/`), with `event.pos`, `event.state`,
  answering the state whose baked model the mesher draws instead. Today the camouflage is kept on
  the server only; clients draw the bare frame.
- `OnBlockExplodedEvent` with the level it happens in (`level: LevelRef`), so an exploded frame
  can drop its camouflage.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
