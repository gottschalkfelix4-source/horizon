# Just Enough Professions (Horizon port)

Port of Just Enough Professions (ATM10 8.x, MIT) as a native Horizon client mod and a plugin of
the [JEI port](../jei/). Written from the mod's observed behaviour and public description, not
from its code. The ATM10 version is to be read from the jar (`horizon-mod.toml` says 1.0.0 until
then).

## What it does

| Feature | Status |
|---|---|
| JEI category "Villager Professions": one recipe per profession, its workstation block, an arrow, a villager of that profession (following the mouse) and the profession's name (vanilla's `entity.minecraft.villager.<profession>`) | ported, not yet checked against the original |
| `U` on a workstation (a lectern) shows the profession it gives; `U` on the villager spawn egg (the category's catalyst) shows them all | ported |
| Vanilla's 13 professions and their job sites | ported |
| Other mods' professions and their workstations (a POI's whole block-state set, several blocks) | needs API (below) |

## Needs from Horizon

- **A profession's job site blocks.** Proposed in `crates/horizon-mod-vocab/src/host/merchants.rs`:
  `fn villager_professions(&self) -> Vec<(String, Vec<BlockKey>)>`: every registered profession
  (`minecraft:villager_profession`) with the blocks of its acquirable job site POI
  (`VillagerProfession.acquirableJobSite` → `PoiType.matchingStates`). `registry_entries` already
  names the professions, but nothing gives their blocks.

## To check against the original (on a machine with the jar)

1. The layout, the category's icon and title, and whether the villager is drawn or only named.
2. Whether a profession with several job-site blocks shows them cycling in one slot.

## Build

Needs the JEI port beside it (`../jei`).

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
