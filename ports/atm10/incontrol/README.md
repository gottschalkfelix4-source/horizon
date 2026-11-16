# InControl (Horizon port)

Port of [InControl](https://www.curseforge.com/minecraft/mc-mods/in-control) 1.21-10.3.0 (ATM10,
MIT, by McJty) as a native Horizon mod. Server-only. Written from the mod's public documentation
of its rule files and observed behaviour, not from its code.

## What it does

Rule files are read from `config/incontrol/` when the server starts and on `/incontrol reload`
(operators, level 2), which also lists every rule it ignored and why. Each file is a JSON list of
rules; all of a rule's conditions must hold, and the first rule that holds decides. A rule using a
condition, result or stage this port does not read yet is ignored as a whole, never half-applied.

| File / feature | Status |
|---|---|
| `spawn.json`, `"when": "position"` (default): `allow` forces the position check to pass, `deny` fails it, `default` leaves vanilla's answer | ported, not yet checked against the original |
| `spawn.json`, `"when": "finalize"`: `deny` stops the spawn before the mob is set up | ported |
| `spawn.json`, `"when": "onjoin"`: `deny` stops the entity joining (not for entities loaded from disk) | ported |
| `spawn.json`, `"when": "despawn"` | not yet |
| `experience.json`: `result: deny`, `setxp`, `multxp`, `addxp` | ported |
| Spawn actions on `allow` (`nbt`, `potion`, equipment, `healthmultiply`, `customname`...) | not yet |
| `loot.json`, `spawner.json`, `summonaid.json`, `potentialspawn.json`, `breeding.json`, phases | not yet |
| `/incontrol show`, `/incontrol list`, `/incontrol kill`, debug commands | not yet |

Conditions read: `mob`, `mod`, `hostile`, `passive`, `dimension`, `biome`, `biometags`,
`minheight`, `maxheight`, `minlight`, `maxlight` (block light or sky light less the sky's
darkening, whichever is higher), `mintime`, `maxtime` (day time modulo 24000), `seesky`,
`weather` (`rain`, `thunder`, `clear`), `difficulty`, `random` (a draw fixed by the tick, position
and mob, so replays agree), `spawner`, `player` (experience: killed by a player), `mincount` and
`maxcount` (a number counts the same mob type in the level; an object takes `amount` and one of
`mob`, `mod`, `hostile`, `passive`, `all`). `mincount: N` holds with at least N, `maxcount: N`
with fewer than N.

## Needs from Horizon

Nothing for the ported part. For the rest:

- `"when": "despawn"`: a hook over `Mob.checkDespawn`, e.g.
  `fn mob_despawn(&self, host: &dyn Host, event: &mut MobDespawnEvent<'_>)` with a `result: &mut DespawnResult`
  (the `mob_despawn` hook exists; its fit with InControl's per-rule semantics is to check).
- Spawn actions need `UnspawnedMobEdits` methods for effects and NBT on the mob being finalized
  (`add_effect(mob, effect, duration, amplifier)`, `merge_nbt(mob, Compound)`) beside the
  equipment and attribute edits it has.
- Per-player and per-chunk counts (`perplayer`, `perchunk`) need cheap counts by area:
  `LevelRef::count_entities_in(self, host, min, max, filter) -> usize`.

## To check against the NeoForge original (oracle on a machine with the jar)

1. Whether a rule with an unknown key is ignored (here) or the key alone is.
2. `mincount`/`maxcount` bounds (inclusive or not) and what a bare number counts.
3. The light the light conditions read, and whether `hostile`/`passive` follow the mob category
   (here) or the entity's class.
4. Whether `onjoin` also applies to entities loaded from disk.

## Build

```sh
CARGO_TARGET_DIR=/home/user/atm10-target cargo test      # in this folder
horizon-launcher native-build . --instance <instance> --trust-native-code
```
