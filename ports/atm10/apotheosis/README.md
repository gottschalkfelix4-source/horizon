# Apotheosis (Horizon port)

Port of [Apotheosis](https://github.com/Shadows-of-Fire/Apotheosis) 8.9.0 (ATM10 8.x, MIT) as a
native Horizon mod. On 1.21 Apotheosis is the adventure module (affix loot, gems, bosses); its
enchanting, spawner and potion modules are the separate Apothic mods. Written from the mod's
observed behaviour and public description, not from its code or data. First slice: affix loot.

## What it does

| Feature | Status |
|---|---|
| Affix loot drops: a monster killed by a player (recently hit) drops an affix item with chance 7.5 % | partly: the original equips mobs with affix gear at spawn and makes them drop it; the port rolls the drop at death |
| Rarities common, uncommon, rare, epic, mythic, weighted, shifted by the killer's luck | ported, numbers from memory |
| Attribute affixes on swords/axes (damage, attack speed, movement speed, knockback, luck) and armour (armour, toughness, max health, movement speed, knockback resistance), value from rarity and a rolled level | partly: ten affixes; the original has many more per category |
| Affixes kept as rarity + level per affix (`minecraft:custom_data` → `apotheosis`), modifiers computed when read (`item_attribute_modifier`), the base item's own modifiers kept | ported |
| Name "<Rarity> <item>" in the rarity's colour (`minecraft:item_name`, translation keys in `assets/apotheosis/lang/en_us.json`) | partly: the original names items with affix prefixes and suffixes |
| Non-attribute affixes (on-hit effects, damage types, durability, crit, life steal: these need Apothic Attributes' attributes) | not yet |
| Gems, sockets, the Reforging / Salvaging / Gem Cutting tables, Sigils | not yet |
| Bosses, invaders, Gateways tie-ins, the Augmenting table | not yet |
| Affix loot in chests (loot table injection) | not yet (`loot_table_load` exists for it) |
| Bows, crossbows, shields, tridents as affix bases | not yet |
| Config and data-driven affixes / rarities (`data/apotheosis/affixes`, `rarities`) | not yet: fixed tables in `src/lib.rs` |

## To check against the NeoForge original

Every number is from memory:

1. The drop chance (the port: 7.5 %), whether only monsters count, and whether a player kill is
   required.
2. Rarity weights and how luck shifts them (the port: weights 400/320/150/90/40 and +-15/-5/+5/+10/+15
   per point of luck, as loot entries' quality works), and the rarity colours.
3. Affixes per rarity (the port: 1 to 5), the affix ids, attributes, operations and value ranges.
4. The stored form: the original keeps affixes in its own data components; the port uses
   `minecraft:custom_data`, so a stack does not move between the two.
5. Whether vanilla gear is the base, and which items.

## Needs from Horizon

- Changing a mob's equipment as it spawns, to give the affix item to the mob as the original
  does: `finalize_spawn` gets `&dyn Host` (read-only). Proposed: pass `&mut dyn ServerHost` in
  `Hooks::finalize_spawn` (`crates/horizon-mod-vocab/src/hooks/finalize_spawn.rs`), so
  `LivingRef::set_equipment` can be called on the new mob.
- A guaranteed drop for that slot: `LivingRef::set_drop_chance(self, host: &mut dyn ServerHost, slot: EquipmentSlot, chance: f32) -> bool`
  (vanilla's `Mob.setDropChance`), in `crates/horizon-mod-vocab/src/host/living.rs`.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
