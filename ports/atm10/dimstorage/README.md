# DimStorage (Horizon port)

Port of DimStorage 9.1.1 (ATM10 8.x, AGPL-3.0-only, so this port is AGPL-3.0-only too) as a native
Horizon mod. Written from the mod's observed behaviour and public description, not from its code.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| Dimensional Chest | 54 slots shared by every chest of the same frequency, in every dimension; vanilla 9x6 menu; hoppers work on the block entity's copy | ported |
| Dimensional Tank | 16 buckets shared by frequency; buckets and the fluid capability | ported |
| Frequency | A number 0..9999; locked frequencies belong to the player who locked them, open only for them and are separate storages from the public one of the same number | ported, set without a screen (below) |
| Dimensional Tablet | Sneak-use on a chest binds it; use opens that chest frequency's contents from anywhere (clicks carried out by the mod) | ported |
| Shared storage persistence | Saved under `horizon-mod-data/dimstorage/` at the end of a changing tick | ported |
| Dimensional Controller (energy for the storages) | | not yet |
| Recipes, models, lid animation, sounds, frequency screen | | not yet (assets / client) |

Setting a frequency, as a stand-in for the original's screen: an ender pearl in hand steps the number
up by one, an eye of ender by ten (down while sneaking); an empty hand while sneaking toggles the lock.

## Needs from Horizon

- A mod screen with a number field and toggle buttons (frequency, lock), instead of the item stand-ins above.
- A block entity container backed by a shared store, so two blocks of one frequency edited in the same tick do
  not lose an edit: today each block keeps a copy that is reconciled every tick, and the later one loses its edit.
- A menu over a mod-supplied item list with the game moving the stacks (the tablet carries out each click itself).

## To check against the NeoForge original

1. Chest size (54) and tank capacity (16 buckets).
2. Whether the frequency is a number or something else (name, colours), its range, and what locking does exactly
   (owner only to open? to change?).
3. What the Dimensional Controller does (not ported because unsure) and whether the tablet also opens tanks.
4. Whether breaking a chest drops its items or leaves them in the frequency (ported: they stay).
5. Hardness and blast resistance (`data/dimstorage/horizon/block/*.json` are placeholders).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
