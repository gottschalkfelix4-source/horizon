# Artifacts (Horizon port)

Port of Artifacts 13.2.5 (ATM10 8.x, MIT) as a native Horizon mod, first slice: seven artifacts and their
effects. Written from the mod's observed behaviour and public description, not from its code. Every
number is from memory.

## What it does

| Artifact | Effect in the port | Status |
|---|---|---|
| Night Vision Goggles (head) | Night vision while worn | ported, not yet checked against the original |
| Snorkel (head) | Water breathing while worn | ported, not yet checked against the original |
| Cross Necklace (chest) | 40 ticks of invulnerability after a hit instead of 20 | ported, not yet checked against the original |
| Running Shoes (feet) | +40 % speed and +0.5 step height while sprinting | ported, not yet checked against the original |
| Bunny Hoppers (feet) | +0.2 jump strength, no fall damage | ported, not yet checked against the original |
| Power Glove (off hand) | +4 attack damage | ported, not yet checked against the original |
| Feral Claws (off hand) | +1 attack speed | ported, not yet checked against the original |
| The other artifacts (about 40: Cloud in a Bottle, Flippers, Fire Gauntlet, Obsidian Skull, Antidote Vessel, Lucky Scarf, Thorn/Flame/Shock Pendants, Golden Hook, Umbrella, Vampiric Glove, Whoopee Cushion, Helium Flamingo, ...) | | not yet |
| Accessory slots (Curios / Accessories): wearing artifacts beside armour | | partly: worn in the nearest vanilla slot |
| Mimic (the chest mob) and artifacts in structure loot | | not yet |
| Config (enable/disable artifacts, cooldowns, amounts) | | not yet |

How: each artifact's `ItemPart::inventory_tick` records it as worn when it sits in its own slot (head 103,
chest 102, feet 100, off hand -106; the data files make each equippable there). At the end of every
server tick each wearer's wanted attribute modifiers (`artifacts:<artifact>_<attribute>`, transient) are
reconciled with the applied ones. Effects are topped up once a second (260 ticks, no particles). The
Cross Necklace sets the post-attack invulnerability in `living_damage_pre`.

## To check against the NeoForge original

1. Every amount above (Running Shoes' speed and step height, Bunny Hoppers' jump, Power Glove's damage,
   Feral Claws' speed, the Cross Necklace's ticks) and whether they are added or multiplied.
2. Whether Bunny Hoppers cancel all fall damage or reduce it, and whether the Cross Necklace's bonus
   scales with something.
3. Whether the Snorkel gives unlimited water breathing or a limited time (later versions).
4. Without an accessory mod, does Artifacts let artifacts be worn in vanilla slots at all? (The port
   chooses the nearest vanilla slot; with Curios in ATM10 they go in their own slots.)
5. The items' facts come from `horizon-port-pack carry` on the jar; `data/artifacts/horizon/item/*.json`
   here are placeholders.

## Needs from Horizon

- Accessory slots, as Curios or Accessories give: a player inventory section a mod declares and reads.
  Proposed: `fn declare_accessory_slot(&mut self, id: &str, size: u32) -> Result<(), String>` on the
  `Registrar` (`crates/horizon-mod-vocab/src/parts/`), `fn player_accessories(&self, player: PlayerRef,
  slot: &str) -> Option<Vec<ItemStack>>` on `PlayersBackend` (`crates/horizon-mod-vocab/src/host/players.rs`),
  and `inventory_tick` firing for those slots with their own slot numbers. Worked around with vanilla
  equipment slots.
- Reading a player's worn items inside a combat hook: `fn living_equipment(&self, entity: i32, slot:
  EquipmentSlot) -> Option<ItemStack>` on `LevelRandomHost` (`crates/horizon-mod-vocab/src/world/types.rs`).
  Worked around with a set of wearers updated each tick.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
