# XNet (Horizon port)

Port of XNet 1.21-7.0.7 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's observed
behaviour and public description, not from its code. A focused core: the network, the connectors and
item, fluid and energy channels.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| Net Cable (`xnet:netcable`) | Cables that touch form a network (flood fill, up to 512 cables) | ported |
| Connector (`xnet:connector`) | Faces one block (its `facing`) and hangs on a cable or controller on any other side. Mode off / extract / insert (empty-hand click), kind items / fluids / energy (sneaking click), channel = dye colour (use a dye) | ported, set by click instead of a screen |
| Controller (`xnet:controller`) | Every 10 ticks moves what extracting connectors can give to the inserting connectors of the same channel and kind, highest priority first, never back into the block it takes from. 16 items, 1,000 mB or 1,000 energy per extractor per operation | ported, numbers assumed |
| Cable colours separating networks, facades, advanced connectors and routers (Wireless Router, redstone/logic channels), per-connector filters, speed and stack-size settings, priorities in a screen, RF cost of operations, controller tiers and channel count | | not yet |

## Needs from Horizon

- **Mod screens** for the controller (channels, connector list, per-connector settings). Settings are by click
  today, with priority fixed at 0.
- **Multipart blocks**: in XNet a connector sits on a cable's side, several per cable. Here a connector is a block
  of its own beside the cable, with its own facing; a cable block with per-side attachments needs a
  `Part` over several shapes and hit-side aware clicks (`shapes` selected by a combination of properties,
  the same gap as Modern Dynamics').
- **Capability-aware filters in transfers**: `move_resources` takes a closure, but a per-connector filter needs a
  screen to edit it (the Filter Module of RFToolsBase is ready to read).

## To check against the NeoForge original

1. Operation period, amounts per operation, and whether energy and fluids move at the same cadence as items.
2. Whether insert priority breaks ties by distance, and whether equal priorities round-robin instead of
   filling the first (here: by position).
3. Whether one controller per network is required (here two controllers on one network both run).
4. Whether the controller uses RF for each operation.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
