# Refined Storage JEI Integration (Horizon port: waiting)

What it does in ATM10: lets JEI's "+" (recipe transfer) pull a recipe's ingredients out of a
Refined Storage network when a Grid or Crafting Grid is open, not only from the player's
inventory, marks the ingredients the network holds or can autocraft, and lets "+" with a key held
start an autocrafting request (or fill a Pattern Grid) for what is missing.

Not ported yet: it is a bridge between two mods, and only one side exists.

- **Waits for:** a Horizon port of Refined Storage (its network storage, Grids, Pattern Grid and
  autocrafting).
- **Already there:** the [JEI port](../jei/) with "+" calling `transfer_recipe_id`, which asks the
  open mod menu's `menu_transfer` part first (`crates/horizon-mod-vocab/src/hooks/menu_transfer.rs`):
  a Refined Storage port can answer transfers into its Grid there, reading the network, so most of
  this mod may become part of that port. Marking available/craftable ingredients needs JEI's
  "missing ingredients" display (not yet in the JEI port) and a check answered from the network
  (`transfer_recipe_check` counts lent containers already).

Licence of the original: to be read from the jar before porting; only a permissive or copyleft
licence allows a committed port (docs/modpacks.md, "Licences").
