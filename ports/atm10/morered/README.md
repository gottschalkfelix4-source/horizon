# More Red (Horizon port)

Port of [More Red](https://github.com/Commoble/morered) 6.0.0.3 (ATM10, MIT) as a native Horizon
mod. Written from the mod's observed behaviour and public description, not from its code.

## What it does

| Content | Behaviour | Status |
|---|---|---|
| `morered:red_alloy_wire` | Lies on any face of a block (floor, wall, ceiling). Carries a signal like redstone dust: full power next to a source, 1 less per wire block, wraps around the edge of the block it lies on. Weakly powers every block beside it. | ported, not yet checked against the original |
| Logic gates: `not_gate`, `diode`, `and_gate`, `and_2_gate`, `nand_gate`, `nand_2_gate`, `or_gate`, `nor_gate`, `xor_gate`, `xnor_gate`, `multiplexer` | Sit on any face, output pointing the way the placer looks. Inputs left, back, right; output front, power 15, one redstone tick (2 game ticks) after the inputs change. | ported, not yet checked against the original |
| `latch`, `t_flip_flop`, `pulse_gate` | Latch: left sets, right resets (both: reset). T flip-flop: toggles on the back input's rising edge. Pulse gate: one redstone tick of output on the back input's rising edge. | ported, not yet checked against the original |
| `morered:red_alloy_ingot` | Item only (recipe from the jar) | declared |
| Gate/wire pops off when its support block goes | wire loses that face; gate breaks | ported |
| Adding a second face to an existing wire cell by placing on it | | not yet (state supports it; placement makes one face) |
| Colored and bundled cables, wire posts/plates, soldering table, alternator, gatecrafting plinth | | not yet |
| Rotating a placed gate (wrench/sneak-use) | | not yet |
| Gate strongly powering the block in front (as a repeater does) | | not yet: see below |

How: each block is declared in `data/morered/horizon/block/*.json` on the `minecraft:glass`
template (not a redstone conductor, no behaviour of its own) with its own state properties:
the wire has one boolean per face plus `power` 0..15; a gate has `attach` (the face it sits on),
`rotation` 0..3, `powered`, and `memory`/`input` for the latch, flip-flop and pulse gate. A wire
that hears a neighbour change schedules a tick; that tick walks its whole network (at most 4096
cells), reads the signal each cell gets from blocks that are not wire, and writes every power
that changed at once (`src/wire.rs`). A gate reads its three inputs; when its next state differs
it schedules a tick 2 game ticks later that writes it (`src/gates.rs`). Placing one from its item
makes it read its inputs at once.

Vanilla redstone dust beside a wire is read 1 lower than it gives: dust takes this mod's
signal at full strength, so without that loss dust and wire would hold each other on after
the source goes.

## Needs from Horizon

- `LevelWriteHost::update_neighbors(&mut self, pos: [i32; 3], source: BlockState, update: NeighbourUpdate) -> bool`
  and `LevelWriteHost::neighbor_changed(&mut self, pos: [i32; 3], source: BlockState) -> bool`,
  callable from `BlockPart::tick_block`. Today they exist only on `LevelRef` with a `ServerHost`,
  which a tick part does not get. Without them a gate can't update what sits behind the block in
  front of it (`DiodeBlock.updateNeighborsInFront`), so gates give weak power only: a lamp right
  in front lights, one behind a stone block does not.
- `LevelHost::face_sturdy(&self, pos: [i32; 3], face: Direction) -> Option<bool>`
  (`BlockState.isFaceSturdy`) for the placement and survival check: today a wire or gate
  needs only a non-air block behind it (`LevelRef::block_face_sturdy` needs a `Host`).
- The dropped item when a wire face pops off while other faces stay: `update_shape` can only give
  the new state, so that face's wire is lost (a whole block that goes drops through its loot
  table as usual). Wanted: a drop request on `UpdateShapeEvent`, or `LevelTicksHost::pop_resource(pos, stack)`.

## To check against the NeoForge original (oracle on a machine with the jar)

1. Block ids (the ids above are assumed) and state property names, so saves carry over.
2. Wire: does it lose 1 power per block, or carry it undiminished? Does it power the block it
   lies on strongly (as dust powers the block below), or only weakly? Which diagonals connect?
3. Gates: delay (assumed 2 game ticks), output strength (assumed 15), strong or weak output,
   which side is left on walls and ceilings, which input the multiplexer's back selects, what
   a latch does with both inputs on, the pulse length.
4. Whether wire updates within the same tick (here: one game tick after the change, then the
   whole network at once).
5. Hardness, sounds, shapes, recipes, loot: from `horizon-port-pack carry` on the jar;
   `data/morered/horizon/` here are placeholders for building without it.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
