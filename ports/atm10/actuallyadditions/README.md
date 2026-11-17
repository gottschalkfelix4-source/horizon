# Actually Additions (Horizon port)

Port of Actually Additions 1.3.26 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's
observed behaviour and public description, not from its code. Only the energy machines are ported;
the mod's farming, items, rings, lasers and empowering are not.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| `coal_generator` | Burns any furnace fuel in its slot, 30 FE per tick, 60 000 FE buffer, pushes into neighbours; stops burning while its buffer is full | ported |
| `powered_furnace` | `minecraft:smelting` recipes through the engine, two inputs (slots 0, 1) into two outputs (2, 3), 25 FE per tick, 100 ticks per item | ported, numbers assumed |
| `crusher` | Hard-coded recipes of vanilla items (cobblestone to gravel, gravel to sand, stone to cobblestone, bone to bone meal, blaze rod to blaze powder), 40 FE per tick, 100 ticks, a chance output in slot 2 | partly: recipes are a table, not data |
| Energy | `ModEnergyBuffer` (saved `energy`): machines take 1000 FE per tick on every face, the generator gives on every face | ported |
| Block entity | One type `actuallyadditions:machine`; lit state from working | ported |
| Double crusher, atomic reconstructor, empowerer, battery box, energizer, farmer, laser relays, phantom blocks, rings, drills, all items | | not yet |

## Needs from Horizon

1. A data-driven machine recipe type (a registered `actuallyadditions:crushing` kind that `recipe_for` can
   match for a single-input machine, with chance outputs): the crusher's recipes are a table in `src/rules.rs`.
2. Mod menu screens with energy bars; the machines open the vanilla hopper menu with its five slots.
3. A way to roll a chance from the level's random (`Host` has no random handle for a block tick); the
   crusher hashes the game time and position instead.

## To check against the NeoForge original

1. Every number in `src/rules.rs`: generator output (30 FE per tick) and buffer (60 000), furnace (25 FE/t, 100 ticks), crusher (40 FE/t, 100 ticks), buffers (40 000).
2. Whether the coal generator keeps burning while its buffer is full.
3. The crusher's recipes (dusts, secondary chances) and whether the crusher has two input slots in the base block.
4. Slot layout and which sides hoppers use.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
