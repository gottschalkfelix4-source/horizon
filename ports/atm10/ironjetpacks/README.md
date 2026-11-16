# Iron Jetpacks (Horizon port)

Port of [Iron Jetpacks](https://github.com/BlakeBr0/IronJetpacks) 8.0.11 (ATM10, MIT) as a native
Horizon mod. Written from the mod's public description and observed behaviour, not from its code.

## What it does

| Feature | Status |
|---|---|
| One `ironjetpacks:jetpack` item whose kind is the `ironjetpacks:jetpack_id` component; tiers wood, stone, copper, iron, gold, diamond, emerald, netherite and creative | ported, numbers assumed |
| Worn in the chest slot: hold jump to fly up, accelerating faster while falling or slow, up to the tier's speed | ported |
| Forward/strafe push in the air, faster while sprinting (sprinting costs more energy) | ported |
| Hover mode: sinks slowly without jump, faster while sneaking; jump climbs at the hover speed | ported |
| Engine toggle, hover toggle, throttle up/down keys (Controls, "Iron Jetpacks"); action-bar line on change | ported, default keys assumed |
| Energy (FE) in the `ironjetpacks:energy` component, used per flying tick by the throttle; no fall damage after flight | ported |
| Charging through other mods' chargers (`ITEM_ENERGY` capability on the jetpack) | ported |
| Tooltip (tier, energy, engine, hover) and energy bar | ported |
| Every jetpack, full, in the creative combat tab | ported (the original has its own tab) |
| Components: strap, coils, cells, thrusters, capacitors (items only, from `data/ironjetpacks/horizon/item`) | items declared; their recipes not yet |
| Crafting recipes that build a jetpack from its tier's cell, thruster and capacitor, and upgrade one keeping its energy | not yet |
| Armour points, enchantability, per-tier colours, the HUD (energy %, engine, hover in a screen corner), thrust particles and sound | not yet |
| Data-driven jetpack definitions (packs adding jetpacks), the tech-mod tiers (bronze, steel, enderium...) and config | not yet: the table is fixed in `src/flight.rs` |

How: as in vanilla, the window's player moves itself, so the flight runs on the client:
`movement_input` records jump/sneak/sprint/forward/strafe, `client_tick_pre` reads the worn
stack and calls `ClientHost::push_player`. Each flying tick the client sends `ironjetpacks:thrust`;
the server takes the energy from the worn stack and resets the fall distance. Toggle keys send
`ironjetpacks:toggle`; the server flips the stack's components and answers on the action bar.

## Needs from Horizon

- Setting the local player's vertical velocity directly: `push_player` adds and clamps each axis
  to -4..4, which is fine for jetpack speeds. A `ClientHost::set_player_velocity([f64; 3])` would
  match the original's `setDeltaMovement` more closely (no `entity_velocity` read-modify-write).
- Recipes that make a component-carrying result (the jetpack id from the cell's id) need a
  mod recipe serializer (`parts/recipe_serializers.rs`), to check whether it covers a shaped
  recipe whose result copies a component from an ingredient.

## To check against the NeoForge original (oracle on a machine with the jar)

1. Every number in `src/flight.rs` (`JETPACKS`): capacity, usage, speeds, acceleration, hover
   speeds, sprint multipliers. They are placeholders in the right order of size.
2. The vertical rule: the 2.5x acceleration while `vy < 0.3`, `min(vy + accel, top)` climbing and
   `min(vy + accel, -sink)` hovering; whether throttle scales speed, acceleration and energy alike.
3. Hover with jump and sneak held together; whether hover holds altitude exactly.
4. Whether fall damage is prevented only while flying or for the whole fall after.
5. Default keys, throttle step (here 5 %), and whether creative players use energy (here: yes,
   only the creative jetpack is free).
6. Item and component ids, lang keys (`assets/ironjetpacks/lang/en_us.json` are this port's).
   The item facts come from `horizon-port-pack carry` on the jar;
   `data/ironjetpacks/horizon/item/*.json` here are placeholders for building without it.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
