# Neo Vitae (Horizon port)

Port of Neo Vitae 1.1.38 (ATM10 8.x, MIT), the Blood Magic successor, as a native Horizon mod.
Written from the mod's observed behaviour and public description (Blood Magic wiki and guide
book, how the mod plays), not from its code. Every number below is **from memory** and must be
checked against the original.

## What it does

| Feature | Status |
|---|---|
| Blood Altar, tier 1: a 10,000 LP tank, recorded per chunk (`neovitae:altars`, saved with the chunk) | ported |
| Sacrificial Dagger: right click costs 1 heart (2 health, `minecraft:magic` damage; none in creative) and pours 200 LP into an altar within 2 blocks on each axis | ported |
| Altar infusion: right click puts one recipe input on the altar, an empty hand takes it back; each tick it drinks `consume` LP into progress, out of LP it loses `drain` progress | ported (stone → blank slate 1000 LP 5/t; diamond → weak blood orb 2000 LP 2/t) |
| Altar tiers 2-6 (multiblock of blood runes, pillars, capstones) | not yet |
| Runes (speed, sacrifice, self-sacrifice, capacity, displacement, orb, acceleration, charging) | not yet |
| Soul Network (player-bound LP), Blood Orbs filling it from the altar, binding | not yet |
| Sacrifice of mobs (Dagger of Sacrifice, Well of Suffering) | not yet |
| Incense and Tranquility, Sanguine Sacrificial bonus | not yet |
| Sigils (water, lava, void, air, green grove, holding, ...) | not yet |
| Alchemy Table, Alchemy Array, Hellfire Forge, Demon Will, Tartaric Gems | not yet |
| Rituals (Master Ritual Stone, ritual stones, activation crystals) | not yet |
| Living Armour, Sentient tools, Routing nodes | not yet |
| Altar HUD (Sanguine Scientiem / Divination Sigil read-out) | not yet |

How: the altar's item records the altar on its chunk when placed. A Blood Altar placed some
other way (`/setblock`, a machine) gets its record the first time a player clicks it. The
`level_tick_post` hook ticks every recorded altar of the level, drops records whose block is
gone, and plays the brewing stand sound when an infusion finishes. The finished item stays on the
altar until an empty hand takes it into that hand (dropped at the player's feet if refused).

Left to do on Horizon's side of things: the API already lets a mod declare a block entity type
for its block (`r.block_entity_type`, `BlockEntityPart::block_entity_tick`). Moving the altar onto
one would keep its state on the block and tick it only while loaded; this first slice keeps
torchmaster's chunk-record pattern instead.

## To check against the NeoForge original

1. The dagger: 200 LP per heart; does it hurt when no altar is in reach (this port: yes, the
   blood is lost)? The search area around the player (this port: 2 blocks each axis); the damage
   type and death message (this port: `minecraft:magic`); can it kill; creative behaviour.
2. Tier 1 capacity (10,000) and whether the tank also has a buffer.
3. Recipe numbers (LP, consumption and drain per tick) and whether progress falls back or
   freezes when dry; altar tick rate (every tick here).
4. What the altar accepts (any item, here only recipe inputs) and where a taken item goes
   (the player's hand here). Items keep no components.
5. The blocks' and items' facts (hardness, shape, recipe, model) come from `horizon-port-pack
   carry` on the jar; `data/neovitae/horizon/**.json` are placeholders for building without it.

## Needs from Horizon

None found for this slice. Left to do with the API as it is: declare `neovitae:sacrifice` as a
damage type in `data/neovitae/damage_type/` (no armour, its own death message) once it is
checked that a native mod's data registers it, and move the altar onto a block entity type.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
