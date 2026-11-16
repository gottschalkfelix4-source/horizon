# Wither Skeleton Tweaks (Horizon port)

Port of [Wither Skeleton Tweaks](https://www.curseforge.com/minecraft/mc-mods/wither-skeleton-tweaks)
10.1.1 (ATM10 8.x, MIT, by Shadows_of_Fire) as a native Horizon mod. Written from the mod's
public description, not from its code.

## What it does

| Feature | Status |
|---|---|
| A skeleton that joins the Nether (not loaded from disk) is kept out and a wither skeleton spawns in its place at the next server tick (`SpawnReason::Conversion`) with the skeleton's hand and armour items, so some wither skeletons carry bows | ported, not yet checked against the original |
| A wither skeleton killed by a player that drops no wither skeleton skull drops a `wstweaks:fragment` | ported, not yet checked |
| Nine fragments craft a wither skeleton skull (`data/wstweaks/recipe/wither_skeleton_skull.json`, shaped 3x3) | placeholder recipe: the real one comes from the jar |
| Damaged stone swords and bows are removed from skeleton, stray and wither skeleton drops (undamaged ones stay) | ported, not yet checked |
| `wstweaks:immolation_blade`: a hit on a skeleton, stray, bogged or wither skeleton kills it at once (`minecraft:generic_kill`) and it drops its skull (a wither skeleton skull, else a skeleton skull) | ported, not yet checked |
| The blade's stats (damage, durability, enchantability, any fire), its recipe and texture | not yet: `data/wstweaks/horizon/item/*.json` are placeholders (diamond sword template) until `horizon-port-pack carry` reads the jar |
| Config (conversion on/off, fragment chance, which mobs, removing loot entries) | not yet: fixed to the behaviour above |

How: `entity_join_level` takes the skeleton out and queues it; `server_tick` spawns the wither
skeleton and copies the equipment over its default stone sword. `living_drops` cleans the
drops, adds the fragment or the head. The blade is an `ItemPart` (`hurt_enemy`), and
`living_death` also marks a mob killed with the blade in hand, since a killing hit drops its loot
before `hurt_enemy` runs.

## Needs from Horizon

- Nothing for the parts ported. A clean conversion would use a `Host::mob_convert_to(mob: MobRef,
  kind: &str, keep_equipment: bool) -> Option<EntityRef>` (`Mob.convertTo`), keeping name,
  persistence and effects; the port's take-out-and-respawn keeps only position and equipment.

## To check against the NeoForge original (oracle on a machine with the jar)

1. Which skeletons convert: every new skeleton in the Nether (spawn eggs, `/summon`, spawners
   included) or natural spawns only; and whether conversion happens at spawn (`FinalizeSpawn`)
   or at join.
2. Whether the fragment needs a player kill (here yes, like the skull's own loot condition),
   its count, and whether Looting changes it.
3. Whether damaged weapons are removed from drops or from the loot tables only (here: any
   damaged stone sword or bow in the drops of the three mobs, which includes equipment drops).
4. The Immolation Blade: which mobs it beheads (here the four skeleton kinds), whether it
   kills outright or only adds the head, and any fire effect.
5. Item facts and recipes come from `horizon-port-pack carry` on the jar.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
