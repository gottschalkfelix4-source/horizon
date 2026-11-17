# Mekanism Additions (Horizon port)

Port of Mekanism Additions 10.7 (ATM10 8.x, MIT) as a Horizon mod beside the [Mekanism port](../mekanism/).
Written from observed behaviour, not from the mod's code. Data only for now: `gen_data.py` writes the files.

| Part | Status |
|---|---|
| Plastic, slick, glow, reinforced, transparent plastic blocks and plastic roads in 16 colours; glow panels; balloons (items); obsidian TNT (a block) | declared, placeholders for look and numbers; no stairs, slabs or fences, no recipes |
| Slick plastic's speed, road speed boost, glow panel placement on walls, balloon entities, obsidian TNT's blast | not yet (need code) |
| Baby mobs, walkie-talkie, jetpack-style items | not yet |

## To check against the NeoForge original

1. Hardness and blast resistance of every block (`gen_data.py`, from memory).
2. Which plastic variants exist (stairs, slabs, fences) and their ids.

## Build

```sh
sh ../dev-sdk.sh && cargo test && python3 gen_data.py
```
