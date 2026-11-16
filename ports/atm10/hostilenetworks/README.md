# Hostile Neural Networks (Horizon port)

Port of Hostile Neural Networks 6.5.1 (ATM10 8.x, MIT) as a native Horizon mod. Written from the
mod's observed behaviour and public description, not from its code. The chain is ported for three
mobs only (zombie, skeleton, creeper), with the port's own simplifications at each step.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| `data_model_<mob>` | Keeps `hostile_data` in `minecraft:custom_data`; tiers faulty, basic, advanced, superior, self aware at 0, 6, 24, 100, 400 data | ported, thresholds assumed |
| Learning | A player carrying a data model of a mob gets +1 data on it for each melee kill of that mob (`mob_killed`), up to 400 | simplified: the original needs the model in a Deep Learner and has per-mob data per kill |
| `sim_chamber` | Data model (slot 0, basic tier or better) plus polymer clay (slot 1) make a `prediction_<mob>` (slot 2) in 300 ticks at 256 FE per tick; consumes one clay, keeps the model | ported, numbers assumed |
| `loot_fabricator` | A prediction (slot 0) makes four of the mob's first drop (rotten flesh, bone, gunpowder) in slot 1 in 100 ticks at 256 FE per tick | simplified: no loot selection, one drop per mob |
| Energy | `ModEnergyBuffer` of 200 000 FE, 4000 FE per tick in | ported |
| Deep learner, matter (living, hellish, ...), model accuracy and failure, trial chamber, all other mobs, the loot selection, simulation chamber's matter output and cost per mob, model upgrades, mob drops lists | | not yet |

## Needs from Horizon

1. Mod menu screens (model slot with tier, simulation graph, loot selection grid); the machines open vanilla's hopper menu with five slots, three used.
2. `mob_killed` only tells of melee kills by players (its documentation says other deaths are not told yet): the Deep Learner's kills by arrows, magic and fakeplayer need `living_death_post` with the attacker's player, which today gives only a mob attacker.
3. Data-driven model definitions (`hostilenetworks:data_model` recipe-like data with tiers, sim cost, loot lists) read through a datapack handle, instead of the table in `src/rules.rs`.

## To check against the NeoForge original

1. Tier thresholds (data per tier), data per kill and the accuracy per tier.
2. Simulation chamber and loot fabricator energy and time, and which items they take and make.
3. Whether the original's matter items exist at all in this chain, and the loot tables per mob.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
