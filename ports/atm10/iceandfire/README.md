# Ice and Fire: Community Edition (Horizon port)

Port of Ice and Fire CE 2.0-beta.17 (ATM10 8.x, LGPL-3.0-or-later) as a native Horizon mod, first
slice. Written from the mod's observed behaviour and public description, not from its code. Every
number in `src/lib.rs` is from memory.

## What it does

| Feature | Status |
|---|---|
| Flamed dragonbone sword (`dragonbone_sword_fire`): sets the target on fire (5 s), knockback, +8 damage to ice dragons | ported, not yet checked against the original |
| Iced dragonbone sword (`dragonbone_sword_ice`): freezes the target (10 s), knockback, +8 damage to fire dragons | partly: a mob's AI is paused (`set_ai_paused`) and slowness VII holds players; no ice block drawn around it |
| Lightning dragonbone sword (`dragonbone_sword_lightning`): lightning bolt on the target, knockback, +8 damage to both dragons | ported, not yet checked against the original |
| Dragonsteel and other elemental weapons (dragonsteel fire/ice/lightning, dragon-blood tools) | not yet |
| Dragons (fire, ice, lightning): growth stages, flight, breath, riding, caves and roosts, eggs | not yet |
| Dragon materials: bones, scales, blood, hearts, dragon forges | not yet |
| Other creatures: sea serpents, hydras, cyclopes, gorgons, myrmexes, stymphalian birds, trolls, sirens, death worms, cockatrices, pixies, hippogryphs, amphitheres, ghosts, dread army | not yet |
| Structures and world generation (dragon caves, roosts, gorgon temples, mausoleums, ore and silver/sapphire) | not yet |
| Bestiary, armour sets, Dragon Staff, Summoning Crystals | not yet |

How: the three swords get an `ItemPart` whose `hurt_enemy` applies the element and pushes the target
away from the player (`entity.push`). A frozen mob is tagged `iceandfire.frozen` and thawed after
`level_tick_post` passes its time; one saved while frozen comes back thawed (`entity_join_level`). The dragon bonus is added before armour in
`living_incoming_damage`, from the hit's weapon. The bonus names `iceandfire:fire_dragon` and
`iceandfire:ice_dragon`, which this slice does not add yet, so it only matters once the dragons are
ported.

## To check against the NeoForge original

1. The numbers: fire duration (5 s?), freeze duration (10 s?), dragon bonus (+8?), knockback strength.
2. Whether the lightning sword's bolt can hurt the attacker, and whether it strikes on every hit or by
   chance / only with a sword cooldown.
3. What "frozen" does exactly (no movement, no AI, no attack?) and whether it breaks on damage.
4. Whether the flamed sword also deals extra damage to other cold mobs (strays, ice creatures) and
   the iced sword to fire mobs (blazes, magma cubes).
5. The swords' damage, speed and durability come from `horizon-port-pack carry` on the jar;
   `data/iceandfire/horizon/item/*.json` here are placeholders (diamond swords).

## Needs from Horizon

- Mod living entity types with their own classes (dragons): `GeneralTypesRegistrar::declare_living_entity(
  &mut self, path: &str, category: MobCategory, width: f64, height: f64, attributes: &[(&str, f64)]) ->
  Result<EntityTypeKey, GeneralEntityRegistrationError>` with an `EntityPart` for its tick and AI
  (`crates/horizon-mod-vocab/src/parts/entity_types.rs`). Today `entity_type/*.json` copies only zombies,
  villagers, wandering traders, wolves, cats and parrots.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
