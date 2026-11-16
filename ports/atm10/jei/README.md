# Just Enough Items (Horizon port)

Port of [Just Enough Items](https://github.com/mezz/JustEnoughItems) 19.64.0.454 (ATM10 8.x, MIT,
by mezz) as a native Horizon client mod. Written from JEI's public documentation and observed
behaviour, not from its code.

## What it does

| Feature | Status |
|---|---|
| Item list beside container screens, paged, every registered item (game-drawn panel, `PanelStyle::Viewer`) | ported |
| Search: words in the name, `@mod`, `#tooltip`, `$tag`, `&resource id`, `-exclusion`, `a\|b` alternatives, `"quoted words"` | ported (`$` see below) |
| Search `%creative tab`, `^colour` | not yet: such a word filters nothing (needs API) |
| `@` matches the mod's display name, not only its namespace | needs API |
| `R` over an item (list, inventory slot, recipe slot, bookmark): its recipes; `U`: its uses; left/right click on the list does the same | ported |
| Recipe screen: a tab per category, category and page arrows that wrap, mouse wheel pages, `Backspace` goes back, `Escape` closes | ported |
| Catalysts column beside each category; clicking one shows the whole category, `U` on a catalyst shows its categories first | ported |
| Vanilla categories: crafting (shaped keeps its pattern), stonecutting, smelting, smoking, blasting, campfire, smithing (transform and trim), fuel (smelt count), composting (chance) | ported |
| Brewing, anvil repairs, special crafting (dyeing, fireworks, map copying...) as example recipes, information pages | not yet |
| `+` beside a recipe: move its ingredients into the open grid or a mod machine's menu (`transfer_recipe_id`), Shift for as many as the items allow | ported |
| `+` greyed and missing ingredients marked red (`transfer_recipe_check`) | not yet |
| Bookmarks: `A` toggles, the list at the left of the container window, saved in `horizon-mod-data/jei/bookmarks.txt` | ported |
| Cheat mode `Ctrl+O`: a click gives a stack (left) or one (right) in creative | ported; on a server JEI falls back to `/give`, not yet |
| Recipes the server sends (`recipes_received`) rebuild the index | ported |
| Plugin interface for other ports: `JeiPlugin` (categories, recipes, catalysts, hidden items, an entity drawn in a recipe), `register_plugin` | ported (see below) |
| JEI's config screen, hiding items by Ctrl-click (edit mode), recipe sorting, ghost ingredient drag onto filters, fluids in the list | not yet |

Key mappings are on Options > Controls under Just Enough Items: Show Recipe (`R`), Show Uses
(`U`), Add/Remove Bookmark (`A`), Toggle Cheat Item Mode (`Ctrl+O`).

## Plugin interface for other ports

A port that adds JEI categories (Just Enough Breeding, Just Enough Professions here; Mekanism or
Refined Storage later) declares the dependency in its `horizon-mod.toml`:

```toml
[dependencies]
jei = "^19"
```

and from its `register` hook:

```rust,ignore
jei::register_plugin(std::sync::Arc::new(MyPlugin));

struct MyPlugin;
impl jei::JeiPlugin for MyPlugin {
    fn uid(&self) -> &str { "mymod:jei" }
    fn register_categories(&self, _: &dyn mod_api::Host, r: &mut jei::Registration) {
        r.add_category(jei::Category::new("mymod:crushing", "gui.mymod.crushing", "mymod:crusher", 82, 26));
    }
    fn register_recipes(&self, host: &dyn mod_api::Host, r: &mut jei::Registration) {
        r.add_recipe("mymod:crushing", jei::Display::new(Some("mymod:crush_iron".into()))
            .input([1, 5], vec!["minecraft:raw_iron".into()])
            .arrow([24, 4])
            .slot_counted(jei::Role::Output, [61, 5], vec!["mymod:iron_dust".into()], 2));
    }
    fn register_catalysts(&self, _: &dyn mod_api::Host, r: &mut jei::Registration) {
        r.add_catalyst("mymod:crushing", "mymod:crusher");
    }
}
```

A recipe is plain data: slots (role, position, the items they cycle through, count), lines of
translated text, an optional arrow and an optional living entity (`Display::entity`, drawn as the
inventory draws the player). JEI asks every plugin again whenever it rebuilds (joining a
world, recipes arriving from the server). JEI's Java API has much more (custom drawables, ingredient
types other than items, transfer handlers, GUI handlers); those are not yet.

## Needs from Horizon

1. **Recipe pages in the game-drawn panel for native mods.** The game already has
   `native_panel_recipes` (`crates/horizon-client/src/game/native_hooks/shared.rs`) and
   `PanelEvent::{SlotClicked, Action, Page, Back, Tab}` reach native mods, but no
   `RecipesClientHost` method lets a native mod call it. Proposed, in
   `crates/horizon-mod-vocab/src/host/recipes.rs`:
   `fn panel_recipes(&mut self, mod_id: &str, title: &str, tabs: Vec<Stack>, tab: u32, recipes: Vec<PanelRecipe>);`
   with `PanelRecipe` (slots with role and stacks, results, fluid wells, action button) in
   `client/types.rs`, the public form of `NativeRecipeView`. Until then this port draws its
   recipe screen itself over the container (`screen_render_post`).
2. **An item's tags.** `$` can only find tags some recipe names (read from the recipe files),
   because nothing lists the tags an item is in. Proposed in `crates/horizon-mod-vocab/src/world/tags.rs`:
   `fn item_tags(&self, item: ItemKey) -> Option<Vec<Identifier>>` on `Tags`.
3. **Creative tabs and mod names.** For `%` and for `@` on a mod's display name: proposed
   `fn creative_tabs_of(&self, item: ItemKey) -> Vec<Text>` on `ItemsClientHost`
   (`host/items.rs`) and `fn mod_display_name(&self, namespace: &str) -> Option<String>` on
   `ConfigHost` (`host/config.rs`).

## To check against the original (on a machine with the jar)

1. The recipe screen's exact sizes and positions (JEI's 1.21 layout), tab paging, and whether a
   click beside it closes it.
2. Search details: whether a plain word also matches the mod name or tooltip by default (JEI's
   config "search mode" per prefix), and how `|` combines with `-`.
3. Fuel counts' rounding (`#.##` assumed), the cooking text keys and positions.
4. Bookmark list order (new bookmarks first assumed) and where JEI saves it
   (`config/jei/world/...` in the original).
5. Whether "+" closes the recipe screen after moving the items.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
