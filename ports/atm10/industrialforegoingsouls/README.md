# Industrial Foregoing Souls (Horizon port)

Port of Industrial Foregoing Souls 1.10.7 (ATM10 8.x, MIT) as a native Horizon mod. Written from the
mod's observed behaviour and public description, not from its code.

**Confidence is low.** I could not recall this add-on's block list and recipes with enough certainty. What is here is one
machine with a plausible, self-contained mechanic under the original's name for its main block, the
Soul Surge. Treat the rest of the original as not ported until it is checked against the original.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| Soul Surge (`industrialforegoingsouls:soul_surge`) | Use a spawn egg on it to bind a mob. Every 100 ticks, with 5,000 FE (100,000 FE buffer, 5,000 in per tick), no redstone signal and fewer than 8 entities in the 9x9x4 area around it, spawns one of the bound mob on top of itself | ported as a stand-in, to check |
| Soul Laser Base and lens, Soul Cleaner, souls-related fluids and items, interaction with Industrial Foregoing's machines (Mob Crusher essence, Laser Drills) | | not yet (not recalled) |
| Recipes, models, lang beyond the block's name | | not yet |

## Needs from Horizon

- Experience of a kill and a mob's loot from a block entity (the same gaps as the Industrial Foregoing port's
  Mob Crusher), if the real machines turn kills into souls or fluid.
- A mod screen for energy and the bound mob; today the bound mob is shown by an action bar message on binding.

## To check against the NeoForge original

1. What the Soul Surge (and the rest of the mod) actually does: this port's mechanic is an assumption.
2. The block and item list, and which of them ATM10 uses.
3. Energy cost, cycle length, mob cap and area of any spawning machine.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
