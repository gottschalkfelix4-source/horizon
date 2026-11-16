# Apothic Spawners (Horizon port)

Port of [Apothic Spawners](https://github.com/Shadows-of-Fire/Apothic-Spawners) 1.4.0 (ATM10, MIT)
as a native Horizon mod. Written from the mod's public description (its CurseForge page and the
ATM guide's modifier table), not from its code.

## What it does

Right-click a monster spawner with a modifier item to change it; one item is used up (not in
creative). Quartz in the off hand inverts the change. An item whose stat is already at its cap is
not used and the click goes on as vanilla's.

| Item | Change | Status |
|---|---|---|
| Sugar | Min spawn delay -10 ticks | ported |
| Clock | Max spawn delay -20 ticks | ported |
| Fermented Spider Eye | Spawn count +2 | ported |
| Ghast Tear | Max nearby entities +2 | ported |
| Prismarine Crystals | Required player range +4 | ported |
| Piston | Spawn range +2 | ported |
| Conduit | Ignore spawn conditions (placement and `checkSpawnRules`) | ported |
| Campfire | Mobs spawn burning | ported |
| Turtle Egg | Mobs spawn as babies where they can | ported |
| White Wool | Mobs spawn silent | ported |
| Chorus Fruit | Mobs spawn without AI | ported |
| Echo Shard | Mobs drop their loot twice | ported, amount assumed |
| Pointed Dripstone | Mobs start with 5 % less health | ported |
| Nether Star | Spawner ignores players (works with nobody near) | not yet: stored, but nothing reads it |
| Soul Lantern | Ignore light only | not yet: stored, but nothing reads it |
| Comparator | Spawner needs a redstone signal | not yet: stored, but nothing reads it |
| Silk Touch | Breaking a spawner with it drops the spawner with its mob and numbers | ported |
| Data-driven modifiers (`apothic_spawners:spawner_modifier` recipes), config, tooltips/Jade lines | | not yet: the table is fixed in `src/modifiers.rs` |

How: the six numbers are vanilla's own spawner fields (`MinSpawnDelay`, `MaxSpawnDelay`,
`SpawnCount`, `MaxNearbyEntities`, `RequiredPlayerRange`, `SpawnRange`), written with
`BlockEntityRef::merge_data`. The switches live in a block entity attachment
(`apothic_spawners:extras`). `finalize_spawn` tags a spawner's mob with the switches that concern
it; `entity_join_level` applies them (`set_remaining_fire_ticks`, `set_silent`, `set_no_ai`,
`merge_nbt {Age, IsBaby}`, `set_health`) and removes the tags; the echoing tag stays for
`living_drops`.

## Needs from Horizon

- Ignore Players, Ignore Light, Redstone Control: a server hook asked by `BaseSpawner.serverTick`
  before it spawns, e.g. `fn spawner_tick(&self, host: &dyn Host, event: &mut SpawnerTickEvent)`
  with `{ dimension, pos: BlockPos, player_near: &mut bool, powered: bool, cancel: &mut bool }`,
  and for Ignore Light a light-only override in `mob_spawn_placement_check` (e.g. an
  `ignore_light: &mut bool` field that `Monster.checkMonsterSpawnRules` reads) instead of the
  all-or-nothing `SpawnCheck::Succeed`.
- Silk-touched spawners keep vanilla's fields only: the attachment is lost when the block entity
  goes. Needs `block_drops` to carry the block entity's attachments, or
  `ItemStack` custom data copied back by `on_block_placed_by` (doable here; not done yet).
- A way to show the spawner's stats (NeoForge's Jade/WTHIT plugin or a tooltip on the placed block).

## To check against the NeoForge original (oracle on a machine with the jar)

1. The caps: assumed delays >= 20, spawn count 1..16, nearby 1..32, player range 1..64, spawn
   range 1..32, initial health -95..0 %. Whether min delay may pass max delay (here it may not).
2. Whether a switch applied twice is refused (here: yes, the item is not used) and whether
   Quartz turns it off (here: yes).
3. Burning: how long the fire lasts (here 32767 ticks). Echoing: how many extra drops (here one
   copy of the whole list). Youthful on mobs without a baby form.
4. Whether Silk Touch is needed, whether the drop is configurable, and whether experience is kept.
5. Which hand the item must be in (here: either hand that clicks; Quartz must be in the off hand).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
