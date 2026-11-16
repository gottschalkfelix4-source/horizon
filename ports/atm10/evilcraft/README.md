# EvilCraft (Horizon port)

Port of [EvilCraft](https://github.com/CyclopsMC/EvilCraft) 1.2.98 (ATM10 8.x, MIT) as a native
Horizon mod. Written from the mod's observed behaviour and public description (the Book of Evil,
the wiki), not from its code. The blood amounts are **this port's own numbers** (the mechanic is
from memory) and must be checked against the original.

## What it does

| Feature | Status |
|---|---|
| Blood stains: a dying entity adds blood (10 mB per point of max health, rounded up) to the block it stood on, if that block's top is sturdy; up to 1000 mB per block; saved per chunk (`evilcraft:stains`) | partly: the stain is recorded, the block is not swapped for its blood-stained look |
| Blood Extractor: right click a stained block draws its blood into the extractor (custom data `evilcraft:blood`, up to 5000 mB) and tells the amount on the action bar | partly: blood is an `Int` in custom data, not yet a `evilcraft:blood` fluid answered through `ITEM_FLUIDS`, so tanks cannot fill from it; no random bonus |
| Blood as a fluid, Dark Tank, Blood Infuser, Blood Chest, Sanguinary Pedestal, Purifier | not yet |
| Dark ore and Dark Gems, Dark Power Gems, Hardened Blood | not yet |
| Environmental Accumulator and weather containers | not yet |
| Vengeance Spirits, Werewolves, Poisonous Libelles, Netherfish, Controlled Zombies | not yet |
| Undead trees, Gemstone torches, Spirit Furnace, Spirit Reanimator, Box of Eternal Closure | not yet |
| Broom, Mace of Distortion, Vengeance Ring, Necromancer's Staff, Kineticators and other items | not yet |
| Degradation (Eternal Water...), Exalted Crafter, Display Stand | not yet |

How: `living_death` reads the body's max health and the block under its feet
(`EntityRef::on_block`), checks the block's top face is sturdy, and adds the blood to the record
on that chunk. The extractor's `use_on` (server side) draws a stain at the clicked position into
the stack in the player's hand, in a transaction, and plays a bucket-fill sound. Messages are
translation keys (`assets/evilcraft/lang/en_us.json`).

## To check against the NeoForge original

1. Which deaths stain (all living entities? not players? only with a block below?) and how much
   blood a death gives; per-block maximum; whether stains fade.
2. Which blocks can be stained (the original keeps a list or a tag of stainable blocks).
3. How much the extractor draws per click (the original gives a random amount), its capacity
   and how it shows its fill; whether blood is also drawn from the stain on its own.
4. The items' facts come from `horizon-port-pack carry` on the jar;
   `data/evilcraft/horizon/item/*.json` are placeholders for building without it.

## Needs from Horizon

- **A block that copies another block's look and drops** (the blood-stained block keeps the
  block it replaced): a content-mod block whose block entity stores an inner `BlockState` and
  whose model and drops forward to it, e.g. `"template": "horizon:mimic"` in
  `data/<id>/horizon/block/*.json` plus `LevelRef::set_mimic(self, host: &mut dyn ServerHost,
  pos: [i32; 3], mimic: BlockKey, inner: BlockState) -> Result<bool, LevelWriteError>` in
  `crates/horizon-mod-vocab/src/host/levels.rs`.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
