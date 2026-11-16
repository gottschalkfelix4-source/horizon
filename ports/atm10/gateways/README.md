# Gateways to Eternity (Horizon port)

Port of Gateways to Eternity 5.1.0 (ATM10 8.x, MIT) as a native Horizon mod, first slice. Written from
the mod's observed behaviour and public description, not from its code or data.

## What it does

| Feature | Status |
|---|---|
| Gate Pearl used on a block opens a gateway there and is used up | ported, not yet checked against the original |
| Waves: setup pause, spawn, all mobs dead clears the wave, next wave | ported |
| Wave rewards dropped at the gateway; completion after the last wave | ported (items only) |
| Failure: wave timeout, a mob past the leash range; the rest of the wave is removed | ported |
| Boss bar with the wave number and the mobs left | ported (shown to the opener) |
| Gateways defined in datapacks (`data/*/gateways/*.json`), every gateway ATM10 ships | not yet: two built-in approximations (`gateways:basic/blaze`, `gateways:basic/slime`) |
| Gateway entity (the portal you see and its particles), its size classes | not yet: no visual besides the boss bar |
| Endless gateways, wave modifiers (effects, attributes on mobs), entity NBT in waves | not yet |
| Reward kinds besides items (experience, loot tables, commands, chanced rewards) | not yet |
| Failure penalties, spawn algorithms (open field / inward spiral), player leash | not yet |
| Gateways surviving a server restart | not yet: open gateways live in memory; their spawned mobs stay (persistent). The API has what it takes (a `Holder::Level` attachment, mob UUIDs through `entity.uuid` and `entity_by_uuid`); it is the next step |

How: the pearl's `ItemPart::use_on` opens a gateway. Which one is read from the stack's
`minecraft:custom_data` `{gateway: "<id>"}`, else the first built-in one. The run is a pure state
machine (`Gateway::step`, unit-tested) driven from `level_tick_post` for the gateway's level. Each step
sees how many of the wave's mobs (tagged `gateways.wave_mob`, persistent) are still alive and whether
one strayed. Mobs spawn on a ring around the gateway (`spawn_range`). Texts are translation keys
(`assets/gateways/lang/en_us.json`, the port's own keys).

## To check against the NeoForge original

1. The real gateway definitions in the mod's data (waves, counts, setup and wave times, leash and
   spawn ranges, rewards). The two here are from memory and certainly differ.
2. Where the pearl stores its gateway (a `gateways:gateway` data component?), and whether it opens on a
   block, in the air, or both.
3. What happens on failure (penalty, rewards kept so far?), and whether the leash is checked for
   mobs, players or both.
4. Who sees the boss bar (everyone near, or the opener?).
5. The pearl's facts come from `horizon-port-pack carry` on the jar;
   `data/gateways/horizon/item/gate_pearl.json` is a placeholder.

## Needs from Horizon

- Datapack-defined gateways need nested records: a gateway is a list of waves, each with lists of
  entities and rewards. `Registrar::declare_datapack_registry` takes flat fields only
  (`DatapackFieldKind`: bool, int, float, text, identifier, reference). Proposed: two new variants in
  `crates/horizon-mod-vocab/src/world/datapack_registry.rs`, `DatapackFieldKind::List { item:
  Box<DatapackFieldKind>, max_len: u32 }` and `DatapackFieldKind::Record { fields:
  Vec<DatapackField> }`, with `DatapackValue::List(Vec<DatapackValue>)` and
  `DatapackValue::Record(Vec<(String, DatapackValue)>)`. Worked around with built-in definitions.

The API already has what the next steps need, so they are not gaps: a general entity type with
`EntityPart::general_entity_tick` for the gateway itself (its look needs a client renderer), and a
`Holder::Level` attachment to save open gateways.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
