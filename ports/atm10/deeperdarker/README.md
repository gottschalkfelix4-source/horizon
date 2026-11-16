# Deeper and Darker (Horizon port)

Port of [Deeper and Darker](https://github.com/KyaniteMods/DeeperAndDarker) 1.4.1 (ATM10 8.x,
AGPL-3.0-only) as a native Horizon mod. Written from the mod's observed behaviour and public
description, not from its code. Every fact below is from memory of how the mod plays and must
be checked against the original.

## What it does

| Feature | Status |
|---|---|
| Otherside portal: Heart of the Deep used on a reinforced deepslate frame fills its opening with `deeperdarker:otherside_portal` | ported, not yet checked against the original |
| Travel: a player standing in the portal (80 ticks in survival, at once in creative) goes to `deeperdarker:otherside` at the same x and z; from the Otherside to the Overworld | ported (players only) |
| A return portal (reinforced deepslate, 2x3 opening) built beside the arrival point | partly: a fixed small portal, not a found or matched one |
| Mobs and items through the portal | not yet |
| Portal collapsing when its frame is broken | not yet |
| The Otherside dimension (terrain, biomes, Echo trees, Gloomy caverns) | not yet: its dimension JSON comes from `horizon-port-pack carry` on the jar; without it travel does nothing |
| Sculk Transmitter (open a linked container from afar) | not yet |
| Warden armour and tools, Soul Elytra, Sonorous Staff | not yet |
| Stalker, Sculk Snapper, Shattered, Sculk Leech and other mobs | not yet |
| Ancient city loot changes (Heart of the Deep in the warden's drops, Warden Heart) | not yet: the item placeholder only |

How: the Heart of the Deep's `ItemPart::use_on` searches, from the block in front of the clicked
face, a rectangular opening along x then z (`find_opening`, a nether portal's shape rules) and
fills it. `player_tick_post` counts the ticks each player stands in a portal block and sends them
with `teleport_to_level`, with vanilla's 300-tick portal cooldown; `player_changed_dimension`
builds the way back.

## To check against the NeoForge original (oracle on a machine with the jar)

1. The frame block (only `minecraft:reinforced_deepslate`?) and the largest opening (32 here).
2. Whether the Heart of the Deep is consumed or damaged when it lights a portal (kept here).
3. The coordinate scale between the Overworld and the Otherside (1:1 here) and where the player
   lands in height (same y here).
4. The wait in the portal (80 ticks here, as a nether portal) and the cooldown.
5. How the original builds or finds the portal on the other side.
6. The sound on lighting (the sculk shrieker's shriek here).
7. Block and item facts (`data/deeperdarker/horizon/**`) are placeholders until `horizon-port-pack
   carry` runs on the jar; the portal placeholder uses `minecraft:structure_void` as template so
   vanilla's nether portal behaviour never runs on it.

## Needs from Horizon

- A level's portal shape for mod portals, so a mod portal block collapses when its frame breaks
  and is found from the far side like a nether portal:
  `trait PortalShape { fn find(&self, level: &dyn LevelHost, pos: [i32; 3]) -> Option<Opening>; }`
  with `Registrar::register_portal(block: BlockKey, frame: &str /* block tag */, destination: &str, shape: impl PortalShape)`
  in `crates/horizon-mod-vocab/src/parts/` (beside `dimensions.rs`). Today the port counts ticks
  itself and cannot collapse the portal.
- Cross-level passages: `Passage::destination_level` is refused when crossed (`FarLevel`), so the
  port cannot use passages for travel.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
