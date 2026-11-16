# Modern Dynamics (Horizon port)

Port of [Modern Dynamics](https://github.com/Technici4n/Modern-Dynamics) 0.9.6 (ATM10 8.x,
LGPL-3.0-only) as a native Horizon mod. Written from the mod's observed behaviour and public
description, not from its code. Released under LGPL-3.0-only, as the original (LICENSE-ORIGINAL.md).

## What it does

| Part | Behaviour | Status |
|---|---|---|
| `moderndynamics:item_pipe`, `fluid_pipe` | Block entity `moderndynamics:pipe`; touching pipes of one kind form a network (walked breadth first, at most 1024 pipes) | ported |
| `moderndynamics:servo` attachment | Used on an item or fluid pipe: fits on the first side touching storage of that kind; every 20 ticks pulls up to 8 items / 1000 mB from that side and delivers it to the nearest storage on the network that takes it (fewest pipes away) | ported, to check |
| Energy cables `lv_cable`, `mv_cable`, `hv_cable`, `ev_cable`, `superconductor` | A cable network moves up to 128 / 512 / 2048 / 8192 / 1,000,000 FE per tick from each energy handler beside it into the others (`ENERGY` capability), once per network per tick | ported, rates to check |
| Items travelling visibly through pipes at pipe speed | | not yet: delivery is instant (needs a client renderer) |
| Pipe shapes and connection states (thin pipes, arms, attachments drawn) | | not yet: placeholder full blocks |
| Retriever, filter, inhibitor and other attachments; attachment GUI (amount, redstone mode, round robin, filter lists) | | not yet |
| Taking a servo back off (wrench / sneak-use) and dropping servos when the pipe breaks | | not yet |
| Machines (if any shipped with 0.9.6) | | not yet |

## Needs from Horizon

- A client-side way to draw items moving along pipes (a block entity renderer for a mod block entity with
  synced per-entity item positions), to port the visible transport instead of instant delivery.
- A hit-position-aware `use_on` for thin shapes: the port picks the servo's side itself because the
  placeholder pipe is a full cube; with real pipe `shapes` keyed on several connection properties
  (the shapes map selects by one property today) the clicked arm would say the side.
- `shapes` selected by a combination of state properties (six boolean `north`..`down` connections),
  so a pipe's outline follows its connections.

## To check against the NeoForge original

1. Servo cadence (every 20 ticks) and amounts (8 items, 1000 mB) per operation.
2. Routing: nearest-first, round robin, or random among destinations; whether the source inventory
   can receive its own items.
3. Energy cable tiers and their per-tick rates; whether a cable network has its own buffer.
4. Whether the item and fluid servo are one item or two, and their recipes.
5. That `BlockEntityRef::merge_data` writes the mod block entity's saved compound (`nbt`), which the
   tick reads back as `servos`.
6. Block facts (hardness, recipes, loot) are placeholders in `data/moderndynamics/horizon/` until
   `horizon-port-pack carry` reads them from the jar.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
