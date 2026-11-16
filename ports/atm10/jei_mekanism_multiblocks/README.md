# JEI Mekanism Multiblocks (Horizon port: waiting)

What it does in ATM10: adds JEI categories that show how to build Mekanism's multiblock
structures (Induction Matrix, Dynamic Tank, Thermal Evaporation Plant, Boiler, Fission and
Fusion Reactors, Turbine, SPS...): the blocks each needs and how many for a chosen size, with a
picture of the structure.

Not ported yet: it is a JEI plugin about another mod's blocks, and that mod is not ported.

- **Waits for:** a Horizon port of Mekanism (and Mekanism Generators) with those multiblocks.
- **Already there:** the [JEI port](../jei/) and its plugin interface (`jei::JeiPlugin`,
  `Registration::add_category`/`add_recipe`, `Display` with slots, counts and text lines), enough
  for the block lists per size. The structure picture needs a JEI drawable that renders blocks in
  a recipe (not in the JEI port's `Display` yet; `DrawContext::draw_scene` could carry it).

Licence of the original: to be read from the jar before porting; only a permissive or copyleft
licence allows a committed port (docs/modpacks.md, "Licences").
