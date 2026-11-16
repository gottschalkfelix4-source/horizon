# Construction Sticks (Horizon port)

Port of Construction Sticks 1.5.0 (ATM10, MIT), the continuation of Construction Wand, as a
native Horizon mod. Written from the mod's observed behaviour and public description, not from
its code.

## What it does

| Feature | Behaviour | Status |
|---|---|---|
| Sticks `constructionstick:{wooden,stone,iron,diamond,netherite}_stick` | Durable tools (59 / 131 / 250 / 1561 / 2031), placing at most 9 / 27 / 81 / 243 / 729 blocks a click | ported, numbers assumed |
| Construction mode | Right-click a block face: every block of the same type connected to it in the face's plane (4-neighbour), whose space in front of the face is free (replaceable, no living entity), gets a copy of the clicked block's state in front of it; nearest first | ported, not yet checked against the original |
| Material | The clicked block, paid from the inventory (lowest slot first); a block item in the other hand is placed (its default state) and paid for instead | ported |
| Creative | No blocks taken, no wear, no limit from the inventory | ported |
| Wear | One durability point per block placed | ported |
| Place sound | The placed block's place sound, once per click | ported |
| Modes switched by sneak + scroll / sneak + right-click air (Extension, Angel, Destruction, Transduction), direction lock, "match similar" | | not yet |
| Undo (sneak + right-click to remove the last placement) | | not yet |
| Stick upgrades (cores applied at the smithing table / crafting), settings screen | | not yet |
| Placement preview outline on the client | | not yet |
| Recipes, textures, models | | from `horizon-port-pack carry` on the jar; `data/constructionstick/horizon/item/*.json` are placeholders on the stick template |

## Needs from Horizon

- Placement preview: the client needs the same plan each frame while a stick is held, which needs
  the player's look ray (`ClientHost` hit result) and an outline drawer in `render_level_stage`.
  Both may exist; not attempted in this port, so no gap is claimed.
- Undo history: a per-player attachment would do; not attempted.

## To check against the NeoForge original (oracle on a machine with the jar)

1. The sticks' tiers, durabilities and limits (the numbers above are assumptions).
2. Whether the spread is 4-neighbour (as here) or 8-neighbour, and whether it continues past a
   blocked position.
3. Whether the placed state copies the clicked block (logs keep their axis, as here) or is the
   item's placement state.
4. Whether the off-hand block overrides the material (as here) and which hand is checked first.
5. Wear per block or per click; whether Unbreaking and Mending apply.
6. Whether placement fires `BlockEvent.EntityPlaceEvent` per block (here `place_block` runs
   `setPlacedBy` but no claim/protection check is made).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
