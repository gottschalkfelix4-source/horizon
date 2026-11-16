# Shrink (Horizon port)

Port of Shrink 2.0.1.47 (ATM10 8.x, LGPL-3.0) as a native Horizon mod, first slice. Written from the
mod's observed behaviour and public description, not from its code.

## What it does

| Feature | Status |
|---|---|
| Shrinking Device used in the air: shrinks the user to 0.1 of their size, again to restore | ported, not yet checked against the original |
| Shrinking Device used on a mob: shrinks or restores the mob | ported, not yet checked against the original |
| Shrunk state saved with the entity (survives relog and restart) | ported: a permanent `minecraft:scale` modifier |
| Device energy (FE) and its charge bar | not yet |
| Shrinking other players | not yet (`item_interact_mob` does not cover players) |
| Mob Bottle (catch a shrunk mob, release it) | not yet |
| `/shrink` command, config (scale, allowed entities, energy cost) | not yet: fixed to the defaults below |
| Shrunk-only movement tweaks (climbing walls, slower speed, items' size) | not yet |

How: the device's `ItemPart` answers `use_item` (the user) and `item_interact_mob` (a mob) on the server.
It toggles a modifier `shrink:shrunk` on `minecraft:scale` (`AddMultipliedTotal`, amount
`0.1 - 1`), so an entity scaled by something else keeps its proportion. The game's own scale handling
follows it: hitbox, eye height, step height, reach, fall damage.

## To check against the NeoForge original

1. The default shrunk scale (0.1?) and whether shrinking goes in one step or several.
2. Whether shrinking yourself takes a sneak-click, and whether the device restores or a second item
   (or a sneak-click) is needed.
3. Which mobs can be shrunk (bosses refused?), and the sound and particles of the device.
4. Whether Shrink on 1.21 uses vanilla's scale attribute or its own capability (it changes how other
   scale mods stack with it).
5. The device's facts come from `horizon-port-pack carry` on the jar;
   `data/shrink/horizon/item/shrinking_device.json` is a placeholder.

## Needs from Horizon

- An item energy store for the device's FE: present in the API (`item_capability`, energy
  capability); not used in this slice. Nothing missing for what is ported.
- Using an item on another player: `item_interact_mob` skips players. Proposed: fire it for players
  too, or a `fn item_interact_player(&self, host: HostSide, event: &mut ItemInteractPlayerEvent<'_>) ->
  Option<InteractionResult>` on `ItemPart` (`crates/horizon-mod-vocab/src/hooks/`).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
