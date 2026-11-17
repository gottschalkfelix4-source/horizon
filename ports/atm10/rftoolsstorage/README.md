# RFToolsStorage (Horizon port)

Port of RFToolsStorage 1.21-6.0.5 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's
observed behaviour and public description, not from its code.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| Modular Storage (`rftoolsstorage:modular_storage`) | Three module slots, one input slot, one output slot (vanilla hopper menu; hoppers use input and output). The modules' sizes add up to a number of stack slots; items in the input slot are stored while slots are free; the output slot is kept filled from the selected kind. Sneak-use with an empty hand selects the next kind | ported (`src/vault.rs` is the slot accounting) |
| Storage Modules (`storage_module0..2`) | Add 300 / 1,000 / 3,000 stack slots when in a module slot | ported, numbers to check |
| Contents | Kept in the block entity (`vault`), as counts per item kind | ported; not kept when the block is broken |
| Remote Storage Module, Storage Scanner (RF, lists inventories nearby), the storage's filter and view screen, module upgrade recipes | | not yet |

## Needs from Horizon

- **Modules keep their own contents.** In the original the contents belong to the module, so a module moved to
  another Modular Storage takes its items along. Wanted: a world-level store keyed by an id on the item, or a
  hook that writes the block entity's data into the module stack.
- **Block entity contents on the dropped item**: a drop hook that keeps `vault` when the block is broken.
- **A mod screen** with a scrollable item list (select, extract any amount), search and filter slots; today only
  the selected kind reaches the output slot.
- **Slot rules for a vanilla menu** (only modules in the three module slots); hoppers can put anything there, it
  just adds no space.

## To check against the NeoForge original

1. Module sizes per tier (300 / 1,000 / 3,000 stack slots here).
2. Whether storage slots are counted per stack as here, or the module stores a fixed item count.
3. Whether the Modular Storage needs RF (assumed: no).
4. What happens to items over capacity when a module is removed (here they stay but nothing new fits).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
