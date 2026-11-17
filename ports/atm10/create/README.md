# Create (Horizon port)

Port of [Create](https://github.com/Creators-of-Create/Create) (ATM10 8.x, MIT) as a native Horizon
mod. Written from the mod's observed behaviour and public description, not from its code.
This is stage 1: rotation and stress. Later stages: press, mixer, millstone and crushing wheel
(stage 2), belts, depot and funnels (stage 3), contraptions (stage 4).

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

Consumers exist in the solver (`Part::Consumer`); the machines that use them come in stage 2.

## To check against the NeoForge original

Every figure is from memory of the default config: water wheel 8 RPM and 32 su per RPM, windmill
64 su per RPM, creative motor 16 RPM, the gearbox's sense on corners, which offsets a large and a
small cogwheel mesh at, and the sail rule. `src/rules.rs` and `src/kinetics.rs` hold them.

## Needs from Horizon

1. A client-side block entity renderer for rotating models (shafts, cogwheels turn visibly);
   today the blocks are static placeholders.
2. Right-click hooks with held-item and block-face context for the hand crank and wrench.
3. Contraption entities (stage 4): moving structures of blocks with their own block entities.

## Build

`sh ../dev-sdk.sh`, then `cargo test --lib` and `cargo check` in this folder.
