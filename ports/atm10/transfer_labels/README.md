# Transfer Labels (Horizon port)

Port of Transfer Labels 0.1.9 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's
public description only, not from its code. This port's author had less to go on for this mod than
for the others: the mechanics below are the most plausible reading of that description and are all
listed under "To check".

## What it does

| Part | Behaviour | Status |
|---|---|---|
| `transfer_labels:extract_label`, `transfer_labels:insert_label` | Used on a face of a block with item storage: the label sticks to that face (recorded on the chunk, attachment `transfer_labels:labels`, saved with it) and one label item is used up | ported, to check |
| Channels | A label's channel is its custom name (rename it in an anvil); unnamed labels share one channel | ported, to check |
| Transfer | Every 20 ticks each extract label moves up to 16 items out through its face into insert labels of its channel within 32 blocks, starting with a different insert label each round (round robin) | ported, to check |
| Removing a label | Sneak-use any label on a labelled face | ported (the label is not given back) |
| Label drawn on the container face | | not yet (client) |
| Item filters on labels, fluid / energy labels, label GUI | | not yet |

A label whose container is broken stays recorded and does nothing until a container with storage
stands there again.

## Needs from Horizon

- A face-attached decal (a client part that draws a sprite on a block face from a synced chunk
  attachment), to show the labels.
- A way to tell, from a `query` row of a chunk attachment, which level the chunk is in (or a
  `query` scoped to one `LevelRef`): the port relies on `level_tick_post`'s host only listing the
  ticking level's chunks.

## To check against the NeoForge original

1. The whole mechanic: labels as items stuck on faces, extract/insert kinds, channels by name.
2. Cadence (20 ticks), amount (16 items) and range (32 blocks) of a transfer, and whether range is
   limited at all.
3. Round robin versus nearest-first versus fill-first distribution.
4. Whether removing a label gives the item back.
5. The original's author for `horizon-mod.toml` (`Original: ...`).
6. Item facts and recipes are placeholders in `data/transfer_labels/horizon/` until
   `horizon-port-pack carry` reads them from the jar.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
