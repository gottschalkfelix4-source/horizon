# Super Factory Manager (Horizon port)

Port of [Super Factory Manager](https://github.com/TeamDman/SuperFactoryManager) 4.34.0 (ATM10 8.x,
MPL-2.0) as a native Horizon mod, under the same licence. Written from the mod's public
documentation and observed behaviour, not from its code. Every fact below about how the original
behaves is from memory and has to be checked against it (see the list at the end).

## What it does

The Factory Manager (`sfm:manager`) runs a program written in SFML. Every trigger
(`EVERY n TICKS|SECONDS DO ... END`) that is due moves items: each `OUTPUT` statement takes from
the `INPUT` statements above it in the same trigger. Inventories are named by labels; a labelled
inventory counts only while it touches the manager or an `sfm:cable` connected to it.

```text
NAME "ores"
EVERY 20 TICKS DO
    INPUT 64 iron_ore, *gold* FROM ores
    OUTPUT TO furnace TOP SIDE
END
```

Until the manager has a screen and the Label Gun exists, programs and labels are set with commands:

```text
/sfm program <manager pos> <code>          (the program on one line: SFML ignores line breaks)
/sfm label <manager pos> <name> <inventory pos>
/sfm unlabel <manager pos> <name>
```

| Feature | Status |
|---|---|
| Factory Manager block, ticking its program | ported |
| Cables: inventories reachable through connected cables only | ported (faces only, at most 8192 cables) |
| SFML `NAME`, `EVERY n TICKS/SECONDS DO ... END`, `INPUT`/`OUTPUT`, per-item limits, `*` patterns, `<face> SIDE` | ported |
| Minimum trigger interval of 20 ticks | ported |
| Programs refused with the word they stop at | ported (translation keys `sfm.horizon.error.*`) |
| Manager screen, program disk, Label Gun, Network Tool | not yet: `/sfm` commands instead |
| `RETAIN`, `EXCEPT`, `EACH`, `WITH` tags, `FORGET`, `IF ... THEN`, `EVERY REDSTONE PULSE` | not yet |
| Fluids, energy (FE), chemicals (Mekanism) | not yet |
| Timer upgrades (intervals under 20 ticks), Fancy cables, Water Tank, Printing Press | not yet |
| Labels across more than one manager, label colours, inspection/diagnostics | not yet |

How: the managers of a level, with their program text and labels, are kept in the level
attachment `sfm:managers` (saved with the level). After each level tick, every manager whose
block is still there runs its due triggers; a manager whose block is gone is forgotten. A
trigger is due when its interval divides the level's game time. Each move runs in a transaction
of its own over the item storage capability (`STORAGE`), so a refused insert puts the items back.

## To check against the NeoForge original

1. When a trigger is due: by the level's game time (here), or by a per-manager tick counter.
2. Limits: here an `INPUT` item's limit is a total per run shared by all the `OUTPUT`s below it,
   and an `OUTPUT` item's limit is a total per run across its inputs. The original may count per
   slot, per inventory (`EACH`) or keep a remainder (`RETAIN`) differently.
3. A pattern without a namespace (`iron_ingot`) matches that path in any namespace here; the
   original may default to `minecraft:` or treat patterns as regular expressions.
4. A statement without a side reaches the inventory with no side (the capability's null side);
   the original may default to `TOP`.
5. The minimum interval of 20 ticks, and that seconds are 20 ticks each.
6. Cables: whether a manager reaches inventories touching it directly, and the original's cable
   count limit.
7. The blocks' facts (hardness, model, recipe) come from `horizon-port-pack carry` on the jar;
   `data/sfm/horizon/block/*.json` here are placeholders for building without it.

## Needs from Horizon

- A filter given the host for `transfer_matching`, so a mod can match items by id (which needs
  `ItemKey::id(host)`) without listing a storage itself, as `move_items` in `src/lib.rs` does now:
  `pub fn transfer_matching_with(host: &mut dyn ServerHost, from: &Capability<dyn ItemStorage>,
  to: &Capability<dyn ItemStorage>, filter: impl Fn(&dyn Host, &ItemResource) -> bool, max: u64,
  tx: &Transaction) -> Result<u64, StorageError>` in `crates/horizon-mod-vocab/src/item/storage.rs`.

Not gaps, to-dos here: the manager's program and labels could live on a block entity of its own
(`BlockEntityPartState`) so they drop with the block; the program editor and Label Gun need a
mod menu and an item part, both of which the API has.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
