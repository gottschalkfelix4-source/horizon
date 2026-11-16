# Nature's Aura (Horizon port)

Port of [Nature's Aura](https://github.com/Ellpeck/NaturesAura) 41.10 (ATM10 8.x, MIT) as a
native Horizon mod. Written from the mod's observed behaviour and public description (the Book of
Natural Aura, the wiki), not from its code. The 1,000,000 default and the spot model are **from
memory**; the radii and the generator's numbers are **this port's own** and must be checked
against the original.

## What it does

| Feature | Status |
|---|---|
| Aura in the world: every place has 1,000,000 by default; devices store or drain at drain spots (signed excess) kept per chunk (`naturesaura:spots`, saved); the Aura at a position is the default plus all spots within 20 blocks; storing goes to the lowest spot in range, draining to the highest | ported |
| Flower Generator: every 10 ticks eats one small flower (`#minecraft:small_flowers`) within 3 blocks (1 up/down) and stores 1000 Aura, 100 less for each of its last 10 flowers of the same kind | ported (numbers to check) |
| Environmental Eye: right click tells the Aura here and its percent of default on the action bar | partly: the original shows a HUD bar while held |
| Aura spreading between spots, effects of too much or too little Aura (plants dying, Aura overflow effects) | not yet |
| Natural Altar (infusion crafting) and its multiblock, Offering Table, Tree Ritual, Ancient Sapling | not yet |
| Other generators: Herbivorous Absorber, Moss Generator, Firecracker Gaze, Potion Generator, Chorus Generator, Oak Generator | not yet |
| Aura-powered devices: Powder Manipulator, Field Creator, Hopper Upgrade, Grated Chute, Weather Changer, Spawn Lamp, Animal Generator ... | not yet |
| Aura Cache, Range Visualizer, Bottles of Aura, Aura-imbued Infused Iron tools and armour | not yet |
| Effect powders, the Ender Crate, the Book of Natural Aura | not yet |

How: the generator's item records the generator on its chunk (`naturesaura:flower_generator`
attachment, with the ids of its last 10 flowers). `level_tick_post` runs every recorded
generator of the level once per 10 game ticks: it lists the small flowers in reach, takes one
(picked by turn, not at random), removes it without drops, and stores the yield at the spot
`spot_for` picks. Records whose block has gone are dropped. Reading the Aura uses a snapshot
query of the loaded chunks' spots, so reading never creates a record.

## To check against the NeoForge original

1. The default Aura (1,000,000), the radius the Aura is summed over, and how storing and
   draining choose their spot.
2. The Flower Generator: its reach, its interval, the Aura per flower, how variety is rewarded
   (this port: linear over the last 10 flowers), whether it picks at random, whether tall flowers
   and mod flowers count, and whether it pauses when the Aura is already high.
3. Whether Aura spots spread or decay over time.
4. The blocks' and items' facts come from `horizon-port-pack carry` on the jar;
   `data/naturesaura/horizon/**.json` are placeholders for building without it.

## Needs from Horizon

Nothing this slice could not do with the current API. Left to do: move the generator onto a
declared block entity type (`r.block_entity_type`, `BlockEntityPart::block_entity_tick`, which
the API already has) so its memory lives on the block, and write the client half (the Eye's HUD
bar) as a client part.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
