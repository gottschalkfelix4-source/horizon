# MmmMmmMmmMmm (Horizon port)

Port of MmmMmmMmmMmm 1.21-2.1.2 (ATM10 8.x, CC0-1.0) as a native Horizon mod, first slice. Written
from the mod's observed behaviour and public description, not from its code.

## What it does

| Feature | Status |
|---|---|
| Target Dummy item: used on a block, stands a dummy facing the player | partly: the dummy is a tagged `minecraft:mannequin`, not its own entity |
| The dummy takes every hit and never dies (`/kill` and the void still remove it) | ported, not yet checked against the original |
| Each hit's damage (after armour) shown to the attacker | partly: over the hotbar, not as floating numbers |
| Damage per second over a fight, told in chat when it ends | ported, not yet checked against the original |
| Sneak + empty hand takes the dummy down and drops its item | ported |
| Armour and items worn by the dummy (to test armour) | not yet |
| Dummy as a mob of a chosen type (to test enchantments like Smite), shearing, scarecrow pumpkin | not yet |
| Floating damage numbers with colours per damage type, client options | not yet |
| Config (DPS window, message mode) | not yet: fixed to the defaults below |

How: placing records the dummy's id; dummies loaded from disk are found by their tag in
`entity_join_level`. `living_damage_pre` (after armour, before absorption) records the hit and sets
the health damage to 0. After each level tick the hits are reported: the damage over the attacker's
hotbar, and, once they stop hitting for 3 s, their DPS (`total * 20 / max(ticks from first to last
hit, 20)`) in chat. Texts are translation keys (`assets/dummmmmmy/lang/en_us.json`), and the keys are
the port's own.

## To check against the NeoForge original

1. When a fight ends (3 s without a hit?) and how DPS is measured (first to last hit, or to the end
   of the window?).
2. Whether the shown damage is before or after armour and absorption.
3. Which damage the dummy ignores, and whether its health resets or it just never loses any.
4. The original's translation keys and messages.
5. Whether creative players keep the item when placing (the port always uses one up).
6. Whether `living_damage_pre` fires for a `minecraft:mannequin` in Horizon (it does for mobs; a
   harness should confirm the mannequin is one).
7. The item's facts come from `horizon-port-pack carry` on the jar;
   `data/dummmmmmy/horizon/item/target_dummy.json` is a placeholder.

## Needs from Horizon

- A mod living entity type that is not a mob template: `GeneralTypesRegistrar::declare_living_entity(
  &mut self, path: &str, category: MobCategory, width: f64, height: f64, attributes: &[(&str, f64)]) ->
  Result<EntityTypeKey, GeneralEntityRegistrationError>` (`crates/horizon-mod-vocab/src/parts/entity_types.rs`),
  or `minecraft:mannequin` / `minecraft:armor_stand` accepted as an `entity_type/*.json` template.
  Worked around with a tagged vanilla mannequin.
- Reading an entity's tags inside a combat hook: `fn entity_tags(&self, entity: i32) -> Option<Vec<String>>`
  on `LevelRandomHost` (`crates/horizon-mod-vocab/src/world/types.rs`). Worked around with a set of known
  dummy ids kept from `entity_join_level` / `entity_leave_level`.
- Floating damage numbers need a client mod and a payload; not attempted in this server-side slice.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
