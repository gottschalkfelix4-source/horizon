# Eternal Starlight (Horizon port)

Port of [Eternal Starlight](https://github.com/teaconmc/EternalStarlight) 0.9.1+1.21.1+neoforge
(ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's observed behaviour and public
description, not from its code. The portal's rules below are from memory of how the mod plays
and are the least certain facts of this port: check every one against the original.

## What it does

| Feature | Status |
|---|---|
| Starlight portal: a Blossom of Stars used on a frame fills the air the frame closes in with `eternal_starlight:starlight_portal`; the frame may be of any solid blocks, in any shape, up to 21 blocks across | ported, not yet checked against the original |
| Travel: a player standing in the portal (80 ticks in survival, at once in creative) goes to `eternal_starlight:starlight` at the same x and z, and back to the Overworld | ported (players only) |
| A return portal of the same frame block built beside the arrival point | partly: a fixed 2x3 portal, not a found or matched one |
| Mobs and items through the portal | not yet |
| Portal collapsing when its frame is broken | not yet |
| The Starlight dimension (eternal night, its biomes and terrain) | not yet: its dimension JSON comes from `horizon-port-pack carry` on the jar; without it travel does nothing |
| Bosses (Gatekeeper, Starlight Golem, Lunar Monstrosity, Tangled Hatred) | not yet |
| Crests and mana, Thermal Springstone and Swamp Silver gear, other items | not yet |
| Shooting stars and other Overworld events | not yet |

How: the Blossom of Stars' `ItemPart::use_on` flood-fills, from the block in front of the
clicked face, the air in the x plane then the z plane (`find_opening`); a fluid, a replaceable
block or an opening past 21 blocks across or 441 blocks means the frame is not closed.
`player_tick_post` counts the ticks each player stands in a portal block and sends them with
`teleport_to_level` and vanilla's 300-tick portal cooldown; `player_changed_dimension` builds the
way back from the frame block found under the portal they left.

## To check against the NeoForge original (oracle on a machine with the jar)

1. The activator: `eternal_starlight:blossom_of_stars` here. Is it consumed?
2. The frame: any solid block and any shape here. Is it a fixed block or tag, and a rectangle?
3. The largest opening (21 across, 441 blocks here).
4. The coordinate scale (1:1 here) and the landing height (same y here).
5. The wait and cooldown (80 and 300 ticks here, a nether portal's), and the lighting sound.
6. How the original builds or finds the portal on the far side.
7. Block and item facts (`data/eternal_starlight/horizon/**`) are placeholders until
   `horizon-port-pack carry` runs on the jar; the portal uses `minecraft:structure_void` as
   template so no vanilla portal behaviour runs on it.

## Needs from Horizon

- Mod portals with the engine's portal handling, so the portal collapses when its frame breaks
  and the far side is found like a nether portal's:
  `Registrar::register_portal(block: BlockKey, destination: &str, shape: impl PortalShape)` with
  `trait PortalShape { fn find(&self, level: &dyn LevelHost, pos: [i32; 3]) -> Option<Vec<[i32; 3]>>; }`
  in `crates/horizon-mod-vocab/src/parts/` (beside `dimensions.rs`).
- Cross-level passages (`Passage::destination_level` is refused as `FarLevel` when crossed).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
