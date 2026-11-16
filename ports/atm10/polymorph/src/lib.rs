//! Polymorph for Horizon: when several crafting recipes match the same grid, a button by the
//! result slot opens the results to choose from, and the server crafts the chosen one.
//!
//! Written from Polymorph's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What is ported, what still needs Horizon's API and what has
//! to be checked against the original is in README.md.
//!
//! Server half: `prepare_item_craft` matches the grid against every crafting recipe
//! (`matcher`), tells the player's client the distinct results (`polymorph:options`), and
//! answers the player's pick (`polymorph:select`) as the result. Client half: draws the button
//! and the results over the crafting screen and sends the pick.
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use mod_api::{
    ClientHost, ClientTickPostEvent, Flow, Hooks, Identifier, Ingredient, ItemStack, PayloadId,
    PrepareItemCraftEvent, Recipe, Registrar, RegistrationContext, ScreenClosingEvent,
    ScreenMouseButtonPressedPreEvent, ScreenRenderPostEvent, ScreenView, ServerHost, Text,
    ToClient, ToServer,
};

pub mod choices;
pub mod matcher;

use choices::{Choice, Options};
use matcher::{Grid, Shape};

/// The two payloads, kept from registration.
static OPTIONS: OnceLock<ToClient<Options>> = OnceLock::new();
static SELECT: OnceLock<ToServer<String>> = OnceLock::new();

/// One crafting recipe, ready to match.
struct Crafting {
    id: String,
    shape: Shape<Ingredient>,
    result: ItemStack,
}

/// The server's crafting recipes, rebuilt when the game hands out a new table (a reload).
#[derive(Default)]
struct RecipeCache {
    source: Option<Arc<[Recipe]>>,
    recipes: Vec<Crafting>,
}

/// What the server keeps per player.
#[derive(Default)]
struct PlayerChoice {
    /// The recipe the player last picked.
    picked: Option<String>,
    /// The choices last sent, so an unchanged grid sends nothing.
    sent: Vec<Choice>,
}

#[derive(Default)]
struct Server {
    cache: RecipeCache,
    players: HashMap<u128, PlayerChoice>,
}

static SERVER: Mutex<Option<Server>> = Mutex::new(None);

fn with_server<T>(f: impl FnOnce(&mut Server) -> T) -> T {
    let mut guard = SERVER.lock().unwrap_or_else(|p| p.into_inner());
    f(guard.get_or_insert_with(Server::default))
}

/// A crafting recipe's shape from what the game parsed and its file: shaped and shapeless
/// only; special recipes (map cloning, dyeing armour...) assemble their own results and are
/// left to the game.
fn shape_of(recipe: &Recipe) -> Option<Shape<Ingredient>> {
    let kind = recipe.json.get("type")?.as_str()?;
    match kind {
        "minecraft:crafting_shaped" => {
            let pattern: Vec<String> = recipe
                .json
                .get("pattern")?
                .as_array()?
                .iter()
                .map(|row| row.as_str().map(str::to_owned))
                .collect::<Option<_>>()?;
            matcher::shaped_from_pattern(&pattern, &recipe.ingredients)
        }
        "minecraft:crafting_shapeless" => Some(Shape::Shapeless(recipe.ingredients.clone())),
        _ => None,
    }
}

fn refresh_cache(cache: &mut RecipeCache, host: &dyn mod_api::Host) {
    let table = host.recipes_of_type("minecraft:crafting");
    if cache
        .source
        .as_ref()
        .is_some_and(|old| Arc::ptr_eq(old, &table))
    {
        return;
    }
    cache.recipes = table
        .iter()
        .filter(|recipe| recipe.rule.is_none())
        .filter_map(|recipe| {
            Some(Crafting {
                id: recipe.id.clone(),
                shape: shape_of(recipe)?,
                result: recipe.results.first()?.clone(),
            })
        })
        .collect();
    cache.source = Some(table);
}

/// The client's view of the choices for the open screen.
#[derive(Default)]
struct ClientState {
    choices: Vec<Choice>,
    /// The list of results is open.
    open: bool,
    /// A pick waiting for the next client tick to be sent.
    pending: Option<String>,
}

static CLIENT: Mutex<ClientState> = Mutex::new(ClientState {
    choices: Vec::new(),
    open: false,
    pending: None,
});

fn client() -> std::sync::MutexGuard<'static, ClientState> {
    CLIENT.lock().unwrap_or_else(|p| p.into_inner())
}

/// Size of one result cell in the list, in GUI pixels.
const CELL: f32 = 18.0;

/// Where the toggle button and the result cells go for a result slot box `[x, y, w, h]`: the
/// button just above the slot's right edge, the cells in a row above it, centred on the slot.
fn layout(result: [f32; 4], count: usize) -> ([f32; 4], Vec<[f32; 4]>) {
    let [x, y, w, _] = result;
    let button = [x + w - 10.0, y - 12.0, 12.0, 10.0];
    let row_width = CELL * count as f32;
    let left = x + w / 2.0 - row_width / 2.0;
    let top = button[1] - 2.0 - CELL;
    let cells = (0..count)
        .map(|i| [left + i as f32 * CELL, top, CELL, CELL])
        .collect();
    (button, cells)
}

fn inside(rect: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= rect[0] && at[0] < rect[0] + rect[2] && at[1] >= rect[1] && at[1] < rect[1] + rect[3]
}

/// A crafting screen: a crafting table's, or the survival inventory's 2x2 grid (no menu type,
/// 46 slots). Slot 0 is the result in both.
fn crafting_screen(screen: &ScreenView<'_>) -> bool {
    screen.id == "inventory"
        && match screen.menu {
            Some(menu) => menu == "minecraft:crafting",
            None => screen.slots == 46,
        }
}

/// The open crafting screen's result slot box.
fn result_box(host: &dyn ClientHost) -> Option<[f32; 4]> {
    let menu = host.local_player()?.container_menu(host)?;
    menu.slot_boxes(host)?.first().copied().flatten()
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let mut handlers = r.payload_handlers();
        let mut payloads = handlers.registrar("1");
        let options = payloads.play_to_client_typed::<Options>(
            PayloadId("polymorph:options".into()),
            |options, _| {
                let mut state = client();
                if options.choices != state.choices {
                    state.open = false;
                }
                state.choices = options.choices;
                Ok(())
            },
        );
        let select = payloads.play_to_server_typed::<String>(
            PayloadId("polymorph:select".into()),
            |recipe, context| {
                let player = context.player().map_err(|e| format!("{e:?}"))?;
                with_server(|server| {
                    server.players.entry(player.uuid()).or_default().picked = Some(recipe);
                });
                Ok(())
            },
        );
        let _ = OPTIONS.set(options);
        let _ = SELECT.set(select);
    }

    /// Match the grid against every crafting recipe, offer the distinct results, and give the
    /// player's pick.
    fn prepare_item_craft(
        &self,
        host: &mut dyn ServerHost,
        event: &mut PrepareItemCraftEvent<'_>,
    ) -> Option<ItemStack> {
        let player = host.player(event.player)?;
        let menu = player.container_menu(&*host)?;
        let width = if event.pos.is_some() { 3 } else { 2 };
        let cells: Vec<Option<ItemStack>> = (1..=width * width)
            .map(|slot| menu.slot(&*host, slot).flatten())
            .collect();
        let grid = Grid {
            width,
            height: width,
            cells,
        };
        let (offered, stacks, picked, changed) = with_server(|server| {
            refresh_cache(&mut server.cache, &*host);
            let mut matched = Vec::new();
            let mut stacks = Vec::new();
            for recipe in &server.cache.recipes {
                if matcher::matches(&recipe.shape, &grid, |ingredient, stack| {
                    ingredient.test(&*host, stack)
                }) {
                    matched.push(Choice {
                        recipe: recipe.id.clone(),
                        item: recipe.result.item().id(&*host),
                        count: recipe.result.count() as i32,
                    });
                    stacks.push((recipe.id.clone(), recipe.result.clone()));
                }
            }
            let offered = choices::offer(matched);
            let entry = server.players.entry(event.player).or_default();
            let changed = entry.sent != offered;
            if changed {
                entry.sent = offered.clone();
            }
            (offered, stacks, entry.picked.clone(), changed)
        });
        if changed && let Some(token) = OPTIONS.get() {
            let _ = token.send(
                host,
                player,
                &Options {
                    choices: offered.clone(),
                },
            );
        }
        let index = choices::resolve(&offered, picked.as_deref())?;
        let recipe = &offered[index].recipe;
        stacks
            .into_iter()
            .find(|(id, _)| id == recipe)
            .map(|(_, stack)| stack)
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        let pending = client().pending.take();
        if let (Some(recipe), Some(token)) = (pending, SELECT.get()) {
            let _ = token.send(host, &recipe);
        }
    }

    fn screen_closing(&self, _: &dyn ClientHost, _: &mut ScreenClosingEvent<'_>) {
        let mut state = client();
        state.open = false;
        state.choices.clear();
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        if !crafting_screen(event.screen) {
            return;
        }
        let state = client();
        if state.choices.len() < 2 {
            return;
        }
        let Some(result) = result_box(host) else {
            return;
        };
        let (button, cells) = layout(result, state.choices.len());
        let label = Text::translate("gui.polymorph.toggle", Vec::new());
        let _ = event.draw.widget_button(
            button,
            &label,
            true,
            inside(button, event.screen.mouse) || state.open,
        );
        if !state.open {
            return;
        }
        for (choice, cell) in state.choices.iter().zip(&cells) {
            let hovered = inside(*cell, event.screen.mouse);
            event
                .draw
                .fill(*cell, if hovered { 0xFF8B_8B8B } else { 0xFF37_3737 });
            let Some(item) = Identifier::parse(&choice.item)
                .ok()
                .and_then(|id| host.item_key(&id))
            else {
                continue;
            };
            let at = [cell[0] + 1.0, cell[1] + 1.0];
            if let Some(stack) = ItemStack::new(item, choice.count.max(1) as u32) {
                event.draw.gui_draw_item_with(&stack, at, None);
            }
        }
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        if event.button != 0 || !crafting_screen(event.screen) {
            return Flow::Continue;
        }
        let mut state = client();
        if state.choices.len() < 2 {
            return Flow::Continue;
        }
        let Some(result) = result_box(host) else {
            return Flow::Continue;
        };
        let (button, cells) = layout(result, state.choices.len());
        let at = event.screen.mouse;
        if inside(button, at) {
            state.open = !state.open;
            return Flow::Cancel;
        }
        if !state.open {
            return Flow::Continue;
        }
        if let Some(i) = cells.iter().position(|cell| inside(*cell, at)) {
            state.pending = Some(state.choices[i].recipe.clone());
            state.open = false;
            return Flow::Cancel;
        }
        state.open = false;
        Flow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_result_cells_sit_in_a_row_above_the_button_centred_on_the_slot() {
        // A crafting table's result slot at (124, 35).
        let (button, cells) = layout([124.0, 35.0, 16.0, 16.0], 3);
        assert!(button[1] + button[3] <= 35.0);
        assert_eq!(cells.len(), 3);
        let centre = (cells[0][0] + cells[2][0] + cells[2][2]) / 2.0;
        assert_eq!(centre, 132.0);
        assert!(cells.iter().all(|c| c[1] + c[3] <= button[1]));
    }
}
