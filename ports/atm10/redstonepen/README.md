# Redstone Pen (Horizon port)

Port of [Redstone Pen](https://github.com/stfwi/redstonepen) 1.11.42 (ATM10, MIT) as a native
Horizon mod. Written from the mod's observed behaviour and public description, not from its code.

## What it does

| Content | Behaviour | Status |
|---|---|---|
| `redstonepen:pen` | Right-click a block face: draws a redstone track on that face (in the cell in front of it). Clicking another face of the same cell adds that face. Costs one point of durability per face; free in creative. | ported, not yet checked against the original |
| `redstonepen:quill` | As the pen, paid with one redstone dust from the inventory; fails without one. | ported, not yet checked against the original |
| `redstonepen:track` | Lies on any face(s) of a cell. Carries a signal like redstone dust: full power next to a source, 1 less per cell, wraps around the edge of the block it lies on. Weakly powers every block beside it. Loses a face when its support goes, and goes when no face is left. | ported, not yet checked against the original |
| `redstonepen:relay`, `inverted_relay` | Sits on any face; output front at power 15 while the back is (not) powered, 2 game ticks later. | ported, not yet checked against the original |
| `redstonepen:bistable_relay` | Toggles its output each time the back input turns on. | ported, not yet checked against the original |
| `redstonepen:pulse_relay` | One short pulse each time the back input turns on. | ported, not yet checked against the original |
| Removing a track face with the pen (sneak-use), drawing connections between tracks by hand, track routing rules | | not yet |
| `bridge_relay`, `control_box` (programmable logic), basic lever/button/pulse button | | not yet |
| Refilling the pen with redstone (crafting) | from the jar's recipes | not checked |
| Relay strongly powering the block in front (as a repeater does) | | not yet: see below |

How: the track and relays are declared in `data/redstonepen/horizon/block/*.json` on the
`minecraft:glass` template (not a redstone conductor, no behaviour of its own) with their own
state properties: the track has one boolean per face plus `power` 0..15; a relay has `attach`,
`rotation` 0..3, `powered`, and `memory`/`input` for the bistable and pulse relays. A track that
hears a neighbour change schedules a tick; that tick walks its whole network (at most 4096 cells),
reads the signal each cell gets from blocks that are not track, and writes every power that
changed at once (`src/wire.rs`). A relay reads its back input and, when its next state differs,
schedules a tick 2 game ticks later that writes it (`src/relays.rs`). The pen's click is in
`src/pen.rs`.

Vanilla redstone dust beside a track is read 1 lower than it gives: dust takes this mod's
signal at full strength, so without that loss dust and track would hold each other on after
the source goes.

## Needs from Horizon

- `LevelWriteHost::update_neighbors(&mut self, pos: [i32; 3], source: BlockState, update: NeighbourUpdate) -> bool`
  and `LevelWriteHost::neighbor_changed(&mut self, pos: [i32; 3], source: BlockState) -> bool`,
  callable from `BlockPart::tick_block` (today only `LevelRef` with a `ServerHost` has them). Without
  them a relay can't update what is behind the block in front of it, so relays give weak power only.
- `LevelHost::face_sturdy(&self, pos: [i32; 3], face: Direction) -> Option<bool>`
  (`BlockState.isFaceSturdy`) for the relay's placement and survival check, and the pen's: today
  any non-air block holds a track or relay.

## To check against the NeoForge original (oracle on a machine with the jar)

1. Block and item ids (assumed above) and state property names, so saves carry over.
2. Track: does the signal lose 1 per cell or pass undiminished? Strong or weak power into the
   block it lies on? Which neighbours connect (here: all six sides, and around the edge of a
   shared support)? Does it update in the same tick (here: one game tick later)?
3. Relays: delay (assumed 2 game ticks), output strength (assumed 15), inputs from the sides
   too, pulse length.
4. Pen: durability (placeholder 256), what sneak-use and left-click do, whether a track drops
   or refunds anything when broken (here: nothing).
5. Hardness, sounds, shapes, recipes, loot: from `horizon-port-pack carry` on the jar;
   `data/redstonepen/horizon/` here are placeholders for building without it.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
