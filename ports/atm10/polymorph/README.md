# Polymorph (Horizon port)

Port of Polymorph 1.2.0 by TheIllusiveC4 (ATM10 8.x, LGPL-3.0-or-later) as a native Horizon
mod with a server and a client half (`side = "both"`). Written from the mod's observed behaviour
and public description, not from its code. The port is LGPL-3.0-or-later as well.

## What it does

| Feature | Status |
|---|---|
| Server: on every crafting-grid change (`prepare_item_craft`), match the grid against every shaped and shapeless crafting recipe (data packs included), mirrored shapes too, and find the distinct results | ported |
| Server → client `polymorph:options`: the distinct results (recipe id, item, count) when there are two or more, an empty list when the choice goes away; sent only when they change | ported |
| Client: a small toggle button above the result slot of the crafting table and the inventory's 2x2 grid; it opens a row of the results; clicking one picks it | ported |
| Client → server `polymorph:select`: the picked recipe id; the server remembers it per player and gives that recipe's result while the grid still makes it | ported |
| The result changes as soon as a result is picked | needs API: it changes on the next grid change (see below) |
| Furnace, blast furnace, smoker: choose what an input smelts into, kept on the block entity | needs API |
| Smithing table | not yet: `prepare_smithing` exists; matching smithing transform rules and assembling their result is still to write |
| Special crafting recipes (map cloning, armour dyeing, transmute) | left to the game: their results are assembled, not listed |
| Modded crafting stations (Polymorph's integrations) | not yet |
| Remembering the pick across sessions; the "persistent selection" config | not yet: the pick lives while the server runs |

Without the mod on the client (a vanilla client), the server half changes nothing: it sends
nothing it can't, and no pick ever arrives.

## Needs from Horizon

1. **Refresh a crafting result now.** After a pick arrives the server should prepare the result
   again (vanilla `CraftingMenu.slotsChanged`), but `ItemPlace::MenuSlot` does not reach the
   crafting result and no host call re-runs the preparation. Proposal, in
   `crates/horizon-mod-vocab/src/host/menus.rs` (`MenusServerHost`, `Side: server`):
   `fn refresh_crafting_result(&mut self, player: u128) -> bool` that runs the open crafting
   menu's `slotsChanged` (so `prepare_item_craft` is asked again) and sends the result slot.
2. **Pick a furnace's recipe.** Every furnace hook (`furnace_start_smelt`, `furnace_smelt`,
   `block_cook`) only listens. Proposal: a replace hook in
   `crates/horizon-mod-vocab/src/hooks/furnace_recipe.rs`,
   `fn furnace_recipe(host: &mut dyn ServerHost, event: &mut FurnaceRecipeEvent) -> Option<Identifier>`
   with `FurnaceRecipeEvent { pos: [i32; 3], kind: &str /* minecraft:smelting... */, input: &ItemStack, candidates: &[Identifier] }`,
   asked when `AbstractFurnaceBlockEntity` looks its recipe up; the answer must be one of the
   candidates. The furnace screen's result slot box is already readable (`MenuRef::slot_boxes`).
3. **What the taken result consumes.** When the picked recipe's remainders differ from the
   game's first match (a bucket left behind in one and not the other), the game consumes the
   grid by its own match. Proposal: let `prepare_item_craft` return the recipe id it chose,
   `Option<(ItemStack, Identifier)>`, so the take uses that recipe's remainders.

## To check against the original (on a machine with the jar)

1. The button's place, look and narration; whether the list is a row of items or a grid with
   page buttons when there are many results.
2. Whether a pick is remembered per player, per recipe output or per grid, and for how long.
3. Whether the server sends the choices or the client works them out from synced recipes.
4. Whether it shows when two recipes give the same item with different counts or components.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
