# Luminax (not ported yet: behaviour must be observed first)

Luminax 1.3.1 (ATM10 8.x, NeoForge 1.21.1, LGPL-3.0-or-later, required on client and server).
No code is written here yet, on purpose.

## What is publicly known

The only public description found (a server host's mod page, mirroring the CurseForge listing)
says: "Adds a bunch of simple glowing blocks to the game": single-coloured decorative blocks in
six shapes (full block, slab, stairs, wall, pressure plate, button), written as a replacement
for Antiblocks in All the Mods 10. No author, block list or numbers are given there.

## Why nothing is ported

Everything a port has to get right is missing from that description, and guessing would break
ATM10 worlds or misrepresent the mod:

- **Registry ids.** Saved worlds and ATM10's recipes and quests name the blocks by id
  (`luminax:...`). Guessed ids would load as missing blocks.
- **Colours and count.** Which colours exist (the 16 dyes? more?), and whether each shape
  exists in each colour.
- **Light level and look.** How bright "glowing" is, whether it is emissive (full-bright) only or
  also gives block light, and whether the faces are flat colour or textured. Textures are the
  mod's own assets: they come from the player's jar (`horizon-port-pack carry`), not from here.
- **Behaviour.** Hardness, tool, sounds, whether mobs can spawn on them, the pressure plate's
  and button's timing and what triggers them, recipes and any colour-changing interaction.
- **Why it is required on both sides.** For plain blocks only the registries would explain it;
  if there is more (a config sync, a network payload), it is not described anywhere public.

## What to observe before porting (on a machine with the jar)

1. `horizon-port-pack` inventory of the jar's `data/` and `assets/`: the block ids, block
   states, models, loot tables, recipes and tags. These data files are not code and say most
   of the above.
2. In game: each block's light level (F3), hardness and tool, sounds, spawn behaviour, and the
   button and pressure plate timings, against vanilla's stone ones.
3. Whether the mod opens any network channel (the NeoForge payload list at login).

Then the blocks are likely a content mod (`.hmod`: template blocks with `light`, shapes from
vanilla templates), with a native part only if the observation finds behaviour beyond that.
