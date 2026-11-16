# RFTools Builder (Horizon port)

Port of [RFTools Builder](https://github.com/McJtyMods/RFToolsBuilder) 1.21-7.0.7 (ATM10 8.x, MIT)
as a native Horizon mod. Written from the mod's observed behaviour and public description, not
from its code. Every number below (costs, buffer, order) is from memory and has to be checked
against the original (see the list at the end).

## What it does

The Builder (`rftoolsbuilder:builder`) holds RF in its own buffer (cables fill it from any face;
nothing drains it). Given a shape card it digs out the card's shape, one block per tick and
`RF_PER_BLOCK` (300 RF) each, top layer first:

| Card | What the Builder does with each block |
|---|---|
| Quarry | mines it into the inventory above the Builder, leaves dirt |
| Clear Quarry | mines it into the inventory above the Builder, leaves air |
| Void | destroys it without drops, leaves air |

What doesn't fit in the inventory above (or with none there) drops on top of the Builder. Air,
water, lava, bedrock, unbreakable blocks, blocks with a block entity and the Builder itself are
passed over. When the area is done the card is dropped from the Builder.

Until the card items and the Builder's screen exist, a card is set with a command:

```text
/rftoolsbuilder card <builder pos> quarry|clearquarry|void box|sphere|cylinder|topdome|bottomdome
                     <solid true|false> <width> <height> <length> <dx> <dy> <dz>
/rftoolsbuilder stop <builder pos>
```

`dx dy dz` is the shape's centre relative to the Builder; each side is 1 to 256.

| Feature | Status |
|---|---|
| Builder block with its RF buffer | ported |
| Quarry, Clear Quarry and Void cards | ported (set by command, not items) |
| Shapes box, sphere, cylinder, top dome, bottom dome, solid or hollow | ported |
| Silk Touch and Fortune quarry cards | not yet |
| Shape card (build a shape from blocks in the inventory above), Pump and Liquid cards | not yet |
| Move, copy, swap and back modes (moving a structure between two Space Chambers) | not yet |
| Space Chamber and its card, Composer, Scanner, Projector, Locator | not yet |
| Shapes prism, torus, heart, capped cylinder, composition, scan | not yet |
| Builder screen, redstone control, rotation and mirroring, anchor | not yet |
| Config (costs, speed, buffer) | not yet: the constants in `src/lib.rs` |

How: the Builder's block entity carries the energy buffer the game keeps
(`Registrar::block_energy`); the cards and progress of a level's Builders are in the level
attachment `rftoolsbuilder:builders` (saved with the level). After each level tick every Builder
with a card and the RF for one block looks at up to 256 blocks of its box from its cursor and
works on the first one the card takes; the RF is paid in a transaction before the block goes.

## To check against the NeoForge original

1. RF per block for each card (here 300 for all), the buffer size (here 500 000) and receive rate
   (here 20 000 per insert), blocks per tick (here 1).
2. The quarry's order (here top layer first, then north to south, then west to east) and where
   a shape's box sits relative to the Builder (here the card's offset is the box's centre).
3. Where drops go: the original may use the inventory above, on any side, and may pause when it
   is full instead of dropping items.
4. Which blocks the quarry skips (here air, fluids, bedrock, unbreakable blocks, blocks with a
   block entity) and whether a Quarry card leaves dirt in place of every block.
5. The hollow shapes: here a hollow box is all six faces, a hollow cylinder only its wall.
6. The block's facts (hardness, model, recipe) come from `horizon-port-pack carry` on the jar;
   `data/rftoolsbuilder/horizon/block/builder.json` is a placeholder for building without it.

## Needs from Horizon

- Destroying a block as a transaction step, so a full inventory can roll the dig back instead of
  dropping items (transactions cover block entities and inventories, not block changes today):
  `pub fn destroy_block_in(self, host: &mut dyn ServerHost, tx: &Transaction, pos: [i32; 3],
  tool: Option<&ItemStack>) -> Result<Option<Vec<ItemStack>>, StackWriteError>` on `LevelRef`
  (`crates/horizon-mod-vocab/src/host/levels.rs`, `level_destroy_block`'s area).

Not a gap, a to-do here: the card and cursor could live on the Builder's block entity
(`BlockEntityPartState`) instead of the level attachment, so they move and drop with the block.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
