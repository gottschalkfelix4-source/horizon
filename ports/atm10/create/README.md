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
| Creative motor | 16 RPM (measured, run 23), 16384 su per RPM; turns its shaft the way it faces, so two facing each other jam | ported, speed not configurable yet |
| Networks | The blocks joined by shafts, cogwheels and gearboxes (up to 4096) share one speed. Sources that disagree on speed or sense, or a ring of cogwheels that turns against itself, jam the network. Stress used (impact x RPM of each consumer) above what the sources give (capacity x RPM) stops it | ported; the network's lowest block solves it every 10 ticks and writes `speed_x1000`, `capacity`, `stress`, `jammed`, `overstressed` into every member's block entity |

| Millstone | Four input and five output slots (hoppers work, vanilla 9x1 menu), 4 su per RPM, driven from below. Recipes, outputs, chances and times (ticks at 16 RPM, shorter when faster) from the original's recipe data: stone, grains, bones, dyes, flowers, wool (`src/grind.rs`, core entries without other mods' items) | ported; chances rolled from a position/time hash |
| Depot | One slot a hopper fills and empties | stand-in: no belt, no putting an item on it by hand yet |
| Mechanical press | 8 su per RPM; presses the stack on a depot below it in 1280 RPM-ticks (80 ticks at 16 RPM, unconfirmed): iron, gold, copper and brass ingots to sheets, sugar cane to paper, pulp to cardboard (recipes checked against the original's data) | ported, whole stack at once (to check) |
| Basin, mechanical mixer | Basin: nine slots (hoppers work). Mixer (4 su per RPM) above it mixes in 160 ticks at 16 RPM (unconfirmed: the recipe data has no time, Create's own default is not known to the port): andesite + iron or zinc nugget to andesite alloy; copper + zinc ingot to 2 brass with a heated burner under the basin (recipes from the original's data) | ported; water, milk, honey and lava recipes need fluids |
| Blaze burner | One fuel slot (hoppers, hopper menu); coal and charcoal give "heated" for 1600 ticks, blaze cake "superheated" for 3000 (unconfirmed); put it under a basin; `heat` and `burn` in its saved data | stand-in: no blaze inside, no fuel by hand, no kindling visuals; superheated has no recipe until fluids exist |
| Belt | One item per belt block, moved to the next belt or depot along the horizontal axis across its shaft axis (direction by the sign of the speed), 256 RPM-ticks per block (8 ticks at 32 RPM); hoppers feed and empty it | stand-in: single blocks not a stretched belt, one item each, no funnels, no items riding past a press, stress 0, numbers to check |
| Funnel | Moves one item every 8 ticks from the container behind it to the one it faces (belt, depot, chest, machine) through the items capability | ported, simplified: no filter, no belt-side placement or tunnel, speed from memory |
| Deployer | One held-item slot (hoppers fill it), 4 su per RPM, 160 ticks at 16 RPM (unconfirmed); applies it to the stack on a depot below: planks turn a shaft into a cogwheel and a cogwheel into a large one, an axe scrapes copper one step or removes wax (and keeps itself), honeycomb waxes copper (rules from the original's deploying recipes, checked against all 216 copper ones) | ported, whole stack at once; no sequenced assembly, no other deploying recipes (they are compat or need items not ported yet) |
| Mechanical saw | Cuts the stack on a depot below: andesite alloy into 6 shafts (200 ticks at 16 RPM), bamboo planks into mosaic (20) (cutting recipes of the original), 4 su per RPM (unconfirmed) | stand-in: no belt side, no block or tree cutting, stack must fit 64 after cutting |
| Crushing wheel | A single wheel acts like the millstone (same slots, driven from below, 8 su per RPM) with the original's crushing recipes: ores, raw ores and blocks, tuff, obsidian, netherrack, amethyst, glowstone, blaze rods, wool, with chances and 100-500 tick times | stand-in: the original is a pair of wheels with items falling between them (needs item entities and a controller); zinc and Create's own stone and ore blocks are not in yet, stress is from memory |

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
4. A per-slot stack limit in `ModContainer` (a belt holds one item; hoppers and funnels now put up to a few
   in, because the container takes stacks). Proposed: `slot_limit: u32` on `ModContainer`
   (`crates/horizon-mod-vocab/src/item/containers.rs`), honoured by the items capability.

## Recipes

`data/create/recipe/` (written in the engine's format: ingredients are strings, `#` for tags) holds the original's crafting recipes (MIT, its data files, not code) for the
blocks and items this port has: shafts, cogwheels, gearbox, depot, press, millstone, basin, water wheel,
windmill bearing, andesite alloy, zinc and brass compacting, andesite funnel. Recipes that need blocks not
ported yet (mixer needs the whisk, belts, casings other than andesite) are left out. Not yet run in the game.

The port defines the `c:` item tags its recipes use (`data/c/tags/item/`: zinc ingot and nugget, brass nugget, iron nugget and block, stones). Recipes with zinc or brass blocks wait for those blocks.
