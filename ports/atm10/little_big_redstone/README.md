# Little Big Redstone (Horizon port)

Port of Little Big Redstone 1.9.12-1.21.1 (ATM10 8.x, MIT) as a native Horizon mod. Written from the
mod's observed behaviour and public description (as remembered), not from its code. First slice:
the microchip runs a logic circuit between its four redstone sides.

## What it does

| Feature | Status |
|---|---|
| Microchip block (`little_big_redstone:microchip`): reads redstone on its north, east, south and west sides, runs its circuit, gives a strong 15 on the sides its circuit drives | ported, not yet checked against the original |
| Circuit gates: input, constant, NOT, AND, OR, XOR, NAND, NOR, set-reset latch, toggle (T flip-flop) | ported as Horizon's own gate set; the original's set has to be checked |
| The circuit carried by the item (`minecraft:custom_data` → `program`) and kept with the placed chip (chunk attachment `little_big_redstone:chips`, saved with the chunk, gate memory included) | ported |
| Circuit editor screen (placing and wiring gates on the chip's grid) | not yet: a circuit is written into the item's data (`/give`, see below) |
| Analog (0 to 15) inputs and outputs, comparators/levels inside the circuit | not yet: inputs are on above 0, outputs are 0 or 15 |
| Timers, delays, counters, clocks, and the original's other logic elements | not yet |
| Breaking the chip keeps its circuit in the dropped item | not yet |
| Up and down sides | not yet |

Circuit format: `{gates: [I; op, a, b, ...], outputs: [I; north, east, south, west]}` (-1 for a
side that gives nothing). Ops: 0 input (a = side 0..3 in N, E, S, W order), 1 constant (a = 0/1),
2 NOT a, 3 AND, 4 OR, 5 XOR, 6 NAND, 7 NOR, 8 latch (a set, b reset), 9 toggle a. A gate reads
only gates before it; a circuit that doesn't is refused. A NOT of north driving south:

```
/give @s little_big_redstone:microchip[custom_data={program:{gates:[I;0,0,0,2,0,0],outputs:[I;-1,-1,1,-1]}}]
```

How it runs: a neighbour change schedules the chip's tick one game tick later; the tick steps the
circuit once and writes the outputs into the block state (`out_north` ... `out_west`), which
`block_signal` answers from. Placing the chip runs it once at once.

## To check against the NeoForge original

1. The chip's tick rate and delay from input to output (the port: one game tick).
2. Whether outputs are strong (pass through a block) or weak, and whether they are analog.
3. The gate set, the grid size, and what a blank chip does.
4. The block's facts (hardness, shape, drops): `data/little_big_redstone/horizon/block/microchip.json`
   is a placeholder for what `horizon-port-pack carry` reads from the jar.
5. The author names (see LICENSE-ORIGINAL.md).

## Needs from Horizon

- Read access to attachments where `block_signal` runs. Today `block_signal` gets a read-only
  `&dyn LevelHost`, which has no attachment store, so the chip encodes its outputs in its block
  state; analog outputs that way would need 16^4 states. Proposed, in
  `crates/horizon-mod-vocab/src/world/types.rs`:
  `trait LevelHost { fn attachment_saved(&self, holder: Holder, namespace: &str, name: &str) -> Option<Compound> }`
  (read-only, the saved form), so a part answers a signal from its own record.
- A removal notice with write access for every removal of a block (explosion, piston,
  `/setblock`), not only a player's break (`BlockPart::on_destroyed_by_player` covers that one),
  to forget a chip's record and to put its circuit in the drop:
  `BlockPart::on_removed(&self, level: &mut dyn LevelWriteHost, event: &mut OnRemovedEvent { pos: BlockPos, state: BlockState, new_state: BlockState, moved: bool })`
  (vanilla's `BlockBehaviour.onRemove`). Until then a stale record is harmless: it is replaced
  when a chip is placed there again, and a tick only runs for a chip block.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
