# Time in a Bottle (Horizon port)

Port of [Time in a Bottle](https://www.curseforge.com/minecraft/mc-mods/time-in-a-bottle) 6.5.4
(ATM10 8.x, MIT, by haoict) as a native Horizon mod. Written from the mod's public description
(its costs, durations, limits and command), not from its code.

## What it does

| Feature | Status |
|---|---|
| `tiab:time_in_a_bottle` stores time (component `tiab:stored_time`, in ticks) while carried: 20 ticks each second, up to 622 080 000 | ported, not yet checked against the original |
| Only the first bottle in the inventory (main slots, then armour, then off hand) gains time | ported; assumed, see below |
| Using the bottle on a block (after the block's own use: sneak to use it on a furnace or a chest) spends time and doubles the block's speed for 30 s; each further use doubles again, up to 2^8 = 256x | ported for blocks that tick randomly (crops, saplings, cactus, sugar cane, amethyst, copper ageing...) |
| Cost per use: x2 30 s, x4 30 s, x8 60 s, x16 120 s, x32 240 s, x64 480 s, ... (doubling) | ported (from the documented table) |
| A raised block's 30 s start again at each use | ported; assumed |
| Not enough time, or already at 256x: the use fails, nothing spent | ported |
| Note block harp sound on use, pitch rising with the rate | ported; the pitches are assumed |
| Tooltip with the stored time (`Stored time: Xh Ym Zs`) | ported |
| `/tiab addtime <seconds>`, `/tiab removetime <seconds>` (operator level 2) on the bottle in hand | ported (lowercase literals: Horizon's command words are lowercase, the original's are `addTime`/`removeTime`) |
| Block entities (furnaces, brewing stands, hoppers, spawners, modded machines) ticking faster | not yet: needs a Host method, below |
| The floating rate label (the original's accelerator entity, `x2` ... `x256`) | not yet: the acceleration lives in the mod's memory, not as an entity |
| Accelerations kept across a restart | not yet: in memory, lost when the server stops |
| Config `serverconfig/tiab-server.toml` (`max_stored_time`, `ticks_const`, `each_use_duration`, `max_rate_multi`) | not yet: fixed to the defaults |
| Curio slot and keybind (separate TIAB Curio Support mod) | not in this mod |

How: each accelerated block rolls 2^rate times per server tick against vanilla's random tick
chance (randomTickSpeed 3: 1 in 1365) and runs `LevelRef::block_random_tick` for each hit, the
same path a chunk's own random tick takes (the block's part first, then vanilla's
`randomTick`). The rolls use the mod's own generator, so the level's random sequence is
untouched.

## Needs from Horizon

- **Ticking a block entity on demand**, for furnaces, brewing stands and machines:
  `ServerHost::level_block_entity_tick(&mut self, level: LevelRef, pos: [i32; 3]) -> bool`, run
  the ticker of the block entity at `pos` once (`BlockEntityTicker.tick`, as the level's
  block-entity loop does), false for no ticking block entity there. Exposed on `LevelRef` as
  `block_entity_tick(host, pos)`.
- **Whether a state ticks randomly** (`BlockState.isRandomlyTicking`), so the bottle refuses
  stone and spends nothing on it, as the original does:
  `Host::block_state_is_randomly_ticking(&self, state: BlockState) -> Option<bool>`. Today a
  use on any non-air block spends time.

## To check against the NeoForge original (oracle on a machine with the jar)

1. The gain rule: 20 ticks every second (here at game times divisible by 20), and whether only
   the first bottle gains (assumed, to stop a stack of bottles multiplying time).
2. Whether a further use restarts the 30 s or adds to them, and what a use at 256x does.
3. The random-tick rate for plain blocks (here vanilla's 3/4096 per roll), and which blocks the
   original refuses.
4. Creative players: do they pay?
5. The sound and pitches.
6. The item's facts (texture, rarity, recipe) come from `horizon-port-pack carry` on the jar;
   `data/tiab/horizon/item/time_in_a_bottle.json` is a placeholder (clock template).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
