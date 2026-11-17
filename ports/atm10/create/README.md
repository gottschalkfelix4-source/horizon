# Create (Horizon port)

Port of [Create](https://github.com/Creators-of-Create/Create) (ATM10 8.x, MIT) as a native Horizon
mod. Written from the mod's observed behaviour and public description, not from its code.
Stages: 1 rotation and stress (done), 2 millstone, press, basin, mixer (done), crushing wheel (not yet), 3 belts, funnels and a fuller depot, 4 contraptions.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| Shaft | Carries speed along its axis, both ends | ported |
| Cogwheel, large cogwheel | Carry speed along their axis; small ones mesh with small and large ones beside them (opposite sense, small turns twice as fast as large), large ones mesh diagonally with each other | ported |
| Gearbox | Joins shafts on all six faces; a turn between X and Y faces reverses the sense | ported, sense rule to check |
| Water wheel | 8 RPM while a side (not its axis) touches flowing water | ported, simplified: no flow direction, one speed |
| Windmill bearing | Counts joined wool and sail frames in front of it (up to 64); nothing below 8, then half an RPM per sail up to 16 | stand-in: the original turns a contraption of any blocks |
| Creative motor | 16 RPM, huge capacity | ported, speed not configurable yet |
| Networks | The blocks joined by shafts, cogwheels and gearboxes (up to 4096) share one speed. Sources that disagree on speed or sense, or a ring of cogwheels that turns against itself, jam the network. Stress used (impact x RPM of each consumer) above what the sources give (capacity x RPM) stops it | ported; the network's lowest block solves it every 10 ticks and writes `speed_x1000`, `capacity`, `stress`, `jammed`, `overstressed` into every member's block entity |

| Millstone | Four input and five output slots (hoppers work, vanilla 9x1 menu until its own screen), 4 su per RPM; each item takes 2048 RPM-ticks (64 ticks at 32 RPM): wheat to flour, cobblestone to gravel, bone to 3 bone meal | ported, recipe table and time from memory |
| Depot | One slot a hopper fills and empties | stand-in: no belt, no putting an item on it by hand yet |
| Mechanical press | 8 su per RPM; presses the stack on a depot below it in 1280 RPM-ticks: iron, gold, copper and brass ingots to sheets | ported, whole stack at once (to check) |
| Basin, mechanical mixer | Basin: nine slots (hoppers work). Mixer (4 su per RPM) above it mixes in 2560 RPM-ticks; only recipe so far: iron nugget + andesite to andesite alloy | ported, recipe and time from memory; no heat (blaze burner), no fluids, no compacting |
| Crushing wheel | | not yet: needs item entities between the wheels |

Consumers use the solver's `Part::Consumer`; a millstone or press takes its speed from the
`speed_x1000` the network writes into it.

## To check against the NeoForge original

Every figure is from memory of the default config: water wheel 8 RPM and 32 su per RPM, windmill
64 su per RPM, creative motor 16 RPM, the gearbox's sense on corners, which offsets a large and a
small cogwheel mesh at, and the sail rule. `src/rules.rs` and `src/kinetics.rs` hold them.

## Needs from Horizon

1. Nothing new in the engine for turning models: `block_entity_submit` draws a baked model under
   `rotate_about` with `partial()` and `animation_time()`, and `BlockClick` carries the held item and
   the face. The port does not use them yet (placeholder models, no hand crank, no putting items on a
   depot by hand).
3. Contraption entities (stage 4): moving structures of blocks with their own block entities.

## Build

`sh ../dev-sdk.sh`, then `cargo test --lib` and `cargo check` in this folder.
