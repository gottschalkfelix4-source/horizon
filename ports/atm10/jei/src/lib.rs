//! Just Enough Items for Horizon: the item list beside container screens with JEI's search,
//! "R" for an item's recipes and "U" for its uses on a recipe screen with a tab per category
//! and the blocks that make them, "+" to move a recipe's ingredients into the open grid,
//! bookmarks on "A", and cheat mode (Ctrl+O) that gives the clicked item in creative.
//!
//! Written from JEI's public documentation and observed behaviour, never from its code
//! (docs/modpacks.md, "Licences"). README.md says what is ported and what still has to be
//! checked against the original.
//!
//! Other ports add their own recipe categories through [`JeiPlugin`] and [`register_plugin`].
//!
//! The game draws the item list (`panel_items`, in the viewer's look) and tells the mod about
//! clicks, searches and keys over items (`panel_event`). The recipe screen and the bookmark
//! list are drawn here, over the container screen (`screen_render_post`), so the container
//! stays open for "+".
pub mod bookmarks;
pub mod index;
pub mod plugin;
pub mod search;
pub mod vanilla;
pub mod view;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Instant;

use mod_api::{
    ClientHost, ClientModifiers, ConfigPath, Flow, GuiEntity, GuiEntityLook, GuiEntityView, Hooks,
    Identifier, ItemKey, ItemResource, ItemStack, KeyRegistration, PanelEvent, PanelEventEvent,
    PanelStyle, RecipesReceivedEvent, Registrar, RegistrationContext, ScreenClosingEvent,
    ScreenExclusionAreasEvent, ScreenKeyPressedPreEvent, ScreenMouseButtonPressedPreEvent,
    ScreenMouseScrolledPreEvent, ScreenRenderPostEvent, ScreenView, Stack, Text, TooltipElement,
};

pub use index::Mode;
pub use plugin::{
    Category, Display, EntityShown, JeiPlugin, Line, Registration, Role, Slot, register_plugin,
};

use bookmarks::Bookmarks;
use index::RecipeIndex;
use search::{Entry, Field, Query};
use view::{Geometry, Hit, RecipeView, Rect, inside};

pub const MOD_ID: &str = "jei";

/// The key mappings, listed under JEI on Options > Controls, with JEI's defaults.
const KEY_RECIPES: &str = "show_recipes";
const KEY_USES: &str = "show_uses";
const KEY_BOOKMARK: &str = "bookmark";
const KEY_CHEAT: &str = "toggle_cheat";

const BOOKMARKS_FILE: &str = "bookmarks.txt";

/// Look of the recipe screen (JEI's grey window).
const FRAME: u32 = 0xFFC6C6C6;
const FRAME_EDGE: u32 = 0xFF555555;
const SLOT_BG: u32 = 0xFF8B8B8B;
const HOVER: u32 = 0x80FFFFFF;
const TEXT: u32 = 0xFF404040;
const BUTTON: u32 = 0xFF6F6F6F;
const BUTTON_TEXT: u32 = 0xFFFFFFFF;

/// One item of the list.
struct Item {
    key: ItemKey,
    entry: Entry,
}

/// What JEI builds once it is in a world: the items, the recipes, and the tags the recipes name.
struct Runtime {
    items: Vec<Item>,
    by_id: HashMap<String, usize>,
    index: RecipeIndex,
    /// Item tags the recipes name, for `$` searches.
    tag_names: Vec<String>,
    /// Tags already looked up and written into the entries.
    tags_read: HashSet<String>,
    tooltips_read: bool,
}

/// Something only a `&mut` host can do, kept for the next client tick.
enum Action {
    Transfer { recipe: String, max: bool },
    Give { item: ItemKey, count: u32 },
}

#[derive(Default)]
struct State {
    runtime: Option<Runtime>,
    /// Recipes arrived from the server: build again.
    stale: bool,
    query_text: String,
    shown: Vec<usize>,
    panel_dirty: bool,
    styled: bool,
    bookmarks: Bookmarks,
    bookmarks_loaded: bool,
    bookmarks_dirty: bool,
    view: Option<RecipeView>,
    history: Vec<RecipeView>,
    cheat: bool,
    pending: Vec<Action>,
}

static STATE: Mutex<State> = Mutex::new(State {
    runtime: None,
    stale: false,
    query_text: String::new(),
    shown: Vec::new(),
    panel_dirty: false,
    styled: false,
    bookmarks: Bookmarks::new(),
    bookmarks_loaded: false,
    bookmarks_dirty: false,
    view: None,
    history: Vec::new(),
    cheat: false,
    pending: Vec::new(),
});

fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn started() -> Instant {
    static START: OnceLock<Instant> = OnceLock::new();
    *START.get_or_init(Instant::now)
}

/// The language's text for `key` with `%s` / `%1$s` filled from `args`; the key itself when no
/// language has it, as vanilla shows a missing translation.
fn tr(host: &dyn ClientHost, key: &str, args: &[String]) -> String {
    let pattern = host.translate(key).unwrap_or_else(|| key.to_owned());
    fill(&pattern, args)
}

/// `Component.translatable`'s argument filling: `%s` in order, `%n$s` by position, `%%`.
pub fn fill(pattern: &str, args: &[String]) -> String {
    let mut out = String::new();
    let mut next = 0;
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut digits = String::new();
        while let Some(d) = chars.peek().filter(|d| d.is_ascii_digit()) {
            digits.push(*d);
            chars.next();
        }
        match chars.next() {
            Some('%') if digits.is_empty() => out.push('%'),
            Some('$') if !digits.is_empty() && chars.peek() == Some(&'s') => {
                chars.next();
                let n: usize = digits.parse().unwrap_or(0);
                out.push_str(args.get(n.wrapping_sub(1)).map_or("", String::as_str));
            }
            Some('s') if digits.is_empty() => {
                out.push_str(args.get(next).map_or("", String::as_str));
                next += 1;
            }
            other => {
                out.push('%');
                out.push_str(&digits);
                if let Some(o) = other {
                    out.push(o);
                }
            }
        }
    }
    out
}

/// Walk a recipe file for the tags its ingredients name (`"tag": "c:ingots"`, `"#c:ingots"`).
fn tags_in(json: &mod_api::serde_json::Value, out: &mut HashSet<String>) {
    match json {
        mod_api::serde_json::Value::Object(map) => {
            for (k, v) in map {
                if k == "tag" {
                    if let Some(t) = v.as_str() {
                        out.insert(t.trim_start_matches('#').to_owned());
                    }
                }
                tags_in(v, out);
            }
        }
        mod_api::serde_json::Value::Array(list) => list.iter().for_each(|v| tags_in(v, out)),
        mod_api::serde_json::Value::String(s) if s.starts_with('#') && s.contains(':') => {
            out.insert(s[1..].to_owned());
        }
        _ => {}
    }
}

fn build(host: &dyn ClientHost) -> Option<Runtime> {
    let keys = host.item_keys();
    if keys.is_empty() {
        return None;
    }
    let mut plugins: Vec<Arc<dyn JeiPlugin>> = vec![Arc::new(vanilla::VanillaPlugin)];
    plugins.extend(plugin::plugins());
    let registration = plugin::collect(host, &plugins);
    let hidden: HashSet<&str> = registration.hidden.iter().map(String::as_str).collect();
    let mut items = Vec::new();
    for key in keys {
        let id = key.id(host);
        if id == "minecraft:air" || hidden.contains(id.as_str()) {
            continue;
        }
        let name = host.hover_name(&ItemStack::of(key));
        items.push(Item {
            key,
            entry: Entry::new(id, name),
        });
    }
    let by_id = items
        .iter()
        .enumerate()
        .map(|(i, it)| (it.entry.id.clone(), i))
        .collect();
    let mut tags = HashSet::new();
    for kind in [
        vanilla::CRAFTING,
        vanilla::SMELTING,
        vanilla::BLASTING,
        vanilla::SMOKING,
        vanilla::CAMPFIRE,
        vanilla::STONECUTTING,
        vanilla::SMITHING,
    ] {
        for recipe in host.recipes_of_type(kind).iter() {
            tags_in(&recipe.json, &mut tags);
        }
    }
    let mut tag_names: Vec<String> = tags.into_iter().collect();
    tag_names.sort();
    Some(Runtime {
        items,
        by_id,
        index: RecipeIndex::build(registration),
        tag_names,
        tags_read: HashSet::new(),
        tooltips_read: false,
    })
}

impl Runtime {
    fn key(&self, id: &str) -> Option<ItemKey> {
        self.by_id.get(id).map(|&i| self.items[i].key)
    }

    /// Fill what `query` needs and is not read yet: tooltips, and the members of the tags it
    /// names.
    fn prepare(&mut self, host: &dyn ClientHost, query: &Query) {
        if query.needs(Field::Tooltip) && !self.tooltips_read {
            self.tooltips_read = true;
            for item in &mut self.items {
                let lines = ItemResource::of(&ItemStack::of(item.key))
                    .tooltip(host, false, false)
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|e| match e {
                        TooltipElement::Text(t, _) => host.text_string(&t).ok(),
                        _ => None,
                    })
                    .collect();
                item.entry.tooltip = Some(lines);
            }
        }
        if query.needs(Field::Tag) {
            let wanted: Vec<String> = self
                .tag_names
                .iter()
                .filter(|t| !self.tags_read.contains(*t))
                .filter(|t| {
                    query
                        .texts(Field::Tag)
                        .any(|w| t.to_lowercase().contains(w))
                })
                .cloned()
                .collect();
            for tag in wanted {
                self.tags_read.insert(tag.clone());
                let Ok(id) = Identifier::parse(&tag) else {
                    continue;
                };
                let tags = host.tags();
                let Some(members) = tags.key(&id).and_then(|k| tags.items(k)) else {
                    continue;
                };
                for key in members {
                    if let Some(&i) = self.by_id.get(&key.id(host)) {
                        self.items[i].entry.tags.push(tag.clone());
                    }
                }
            }
        }
    }

    fn filter(&self, query: &Query) -> Vec<usize> {
        (0..self.items.len())
            .filter(|&i| query.matches(&self.items[i].entry))
            .collect()
    }
}

impl State {
    fn refilter(&mut self, host: &dyn ClientHost) {
        let query = search::parse(&self.query_text);
        if let Some(rt) = self.runtime.as_mut() {
            rt.prepare(host, &query);
            self.shown = rt.filter(&query);
            self.panel_dirty = true;
        }
    }

    /// Answer "R" or "U" for `item`; nothing happens when no recipe answers.
    fn open(&mut self, item: &str, mode: Mode) {
        let Some(rt) = &self.runtime else { return };
        if let Some(v) = RecipeView::new(Some(item.to_owned()), mode, rt.index.find(item, mode)) {
            if let Some(old) = self.view.replace(v) {
                self.history.push(old);
            }
        }
    }

    fn open_category(&mut self, cat: usize) {
        let Some(rt) = &self.runtime else { return };
        if let Some(v) = RecipeView::new(None, Mode::Uses, rt.index.all_of(cat)) {
            if let Some(old) = self.view.replace(v) {
                self.history.push(old);
            }
        }
    }

    fn close(&mut self) {
        self.view = None;
        self.history.clear();
    }

    /// An item key went down over `item`: R, U or A as bound.
    fn item_key(&mut self, host: &dyn ClientHost, key: &str, item: &str) -> bool {
        let bound = |name: &str| host.mod_key(MOD_ID, name).map(|(k, _)| k);
        if bound(KEY_RECIPES).as_deref() == Some(key) {
            self.open(item, Mode::Recipes);
        } else if bound(KEY_USES).as_deref() == Some(key) {
            self.open(item, Mode::Uses);
        } else if bound(KEY_BOOKMARK).as_deref() == Some(key) {
            self.bookmarks.toggle(item);
            self.bookmarks_dirty = true;
        } else {
            return false;
        }
        true
    }

    /// A click on an item of the list or the bookmarks: in cheat mode it gives the item (a
    /// stack on a left click, one on a right click), else it shows its recipes (left) or uses
    /// (right).
    fn click_item(&mut self, item: &str, right: bool) {
        if self.cheat {
            if let Some(key) = self.runtime.as_ref().and_then(|rt| rt.key(item)) {
                let count = if right { 1 } else { 64 };
                self.pending.push(Action::Give { item: key, count });
            }
        } else {
            self.open(item, if right { Mode::Uses } else { Mode::Recipes });
        }
    }

    /// Where the recipe screen is this frame, and the recipes on its page.
    fn geometry(&self, size: [f32; 2]) -> Option<(Geometry, Vec<(usize, &Display)>)> {
        let view = self.view.as_ref()?;
        let rt = self.runtime.as_ref()?;
        let cat = view.category();
        let c = &rt.index.categories[cat];
        let g = view::layout(
            size,
            [c.width, c.height],
            view,
            rt.index.catalysts[cat].len(),
        );
        let page = view
            .on_page(g.per_page)
            .iter()
            .filter_map(|&r| rt.index.recipe(cat, r).map(|d| (r, d)))
            .collect();
        Some((g, page))
    }

    fn bookmark_rects(&self, screen: &ScreenView<'_>, size: [f32; 2]) -> Vec<Rect> {
        if !is_container(screen) {
            return Vec::new();
        }
        view::bookmark_slots(size, screen.rect, self.bookmarks.items().len())
    }
}

/// A container window (the inventory, a chest, a machine): JEI shows beside these only.
fn is_container(screen: &ScreenView<'_>) -> bool {
    screen.id == "inventory" || screen.id == "creative" || screen.menu.is_some()
}

/// The item a slot shows now: its alternatives take turns, one a second.
fn cycled(items: &[String]) -> Option<&str> {
    if items.is_empty() {
        return None;
    }
    let n = (started().elapsed().as_secs() as usize) % items.len();
    Some(&items[n])
}

fn draw_button(draw: &mut dyn mod_api::DrawContext, r: Rect, label: &str, hover: bool) {
    draw.fill(r, if hover { 0xFF8F8FBF } else { BUTTON });
    let w = draw.text_width(label);
    draw.text(
        label,
        [r[0] + (r[2] - w) / 2.0, r[1] + 3.0],
        BUTTON_TEXT,
        true,
    );
}

fn draw_frame(draw: &mut dyn mod_api::DrawContext, r: Rect) {
    draw.fill([r[0] - 1.0, r[1] - 1.0, r[2] + 2.0, r[3] + 2.0], FRAME_EDGE);
    draw.fill(r, FRAME);
}

fn draw_arrow(draw: &mut dyn mod_api::DrawContext, at: [f32; 2]) {
    // The furnace's arrow sprite, else a plain one.
    if !draw.sprite(
        "container/furnace/burn_progress",
        [0.0, 0.0, 24.0, 16.0],
        [at[0], at[1], 24.0, 16.0],
        Some(0xFF8B8B8B),
    ) {
        draw.fill([at[0], at[1] + 6.0, 14.0, 4.0], SLOT_BG);
        draw.quad(
            [
                [at[0] + 14.0, at[1] + 2.0],
                [at[0] + 22.0, at[1] + 8.0],
                [at[0] + 22.0, at[1] + 8.0],
                [at[0] + 14.0, at[1] + 14.0],
            ],
            SLOT_BG,
        );
    }
}

impl State {
    fn draw(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        let Some(rt) = &self.runtime else { return };
        let mouse = event.screen.mouse;
        let size = event.draw.size();
        let mut tooltip: Option<ItemKey> = None;

        for (rect, id) in self
            .bookmark_rects(event.screen, size)
            .iter()
            .zip(self.bookmarks.items())
        {
            if let Some(key) = rt.key(id) {
                event.draw.item(key, [rect[0] + 1.0, rect[1] + 1.0], 16.0);
                if inside(*rect, mouse) {
                    event.draw.fill(*rect, HOVER);
                    tooltip = Some(key);
                }
            }
        }

        if let (Some(view), Some((g, page))) = (&self.view, self.geometry(size)) {
            let draw = &mut *event.draw;
            let cat = view.category();
            let category = &rt.index.categories[cat];
            // Tabs over the window.
            for (tab, r) in &g.tabs {
                let selected = *tab == view.tab;
                draw.fill(*r, if selected { FRAME } else { SLOT_BG });
                let tab_cat = &rt.index.categories[view.found[*tab].0];
                if let Some(key) = rt.key(&tab_cat.icon) {
                    draw.item(key, [r[0] + 3.0, r[1] + 3.0], 16.0);
                }
            }
            draw_frame(draw, g.frame);
            // Catalysts in their column.
            if !g.catalysts.is_empty() {
                let col = view::bounds(&g.catalysts).unwrap_or_default();
                draw_frame(
                    draw,
                    [col[0] - 2.0, col[1] - 2.0, col[2] + 4.0, col[3] + 4.0],
                );
            }
            for (r, id) in g.catalysts.iter().zip(&rt.index.catalysts[cat]) {
                draw.fill(*r, SLOT_BG);
                if let Some(key) = rt.key(id) {
                    draw.item(key, [r[0] + 1.0, r[1] + 1.0], 16.0);
                    if inside(*r, mouse) {
                        tooltip = Some(key);
                    }
                }
            }
            // Title, page, and their arrows.
            let title = tr(host, &category.title, &[]);
            let w = draw.text_width(&title);
            draw.text(&title, [g.title[0] - w / 2.0, g.title[1]], TEXT, false);
            let pages = view.pages(g.per_page).max(1);
            let label = format!("{}/{}", view.page + 1, pages);
            let w = draw.text_width(&label);
            draw.text(
                &label,
                [g.page_label[0] - w / 2.0, g.page_label[1]],
                TEXT,
                false,
            );
            for (r, label) in [
                (g.prev_category, "<"),
                (g.next_category, ">"),
                (g.prev_page, "<"),
                (g.next_page, ">"),
            ] {
                draw_button(draw, r, label, inside(r, mouse));
            }
            // The recipes.
            for ((origin, plus), (_, display)) in g.recipes.iter().zip(&page) {
                if let Some(e) = &display.entity {
                    let (x, y) = (origin[0] + e.rect[0] as f32, origin[1] + e.rect[1] as f32);
                    let rect = [x, y, x + e.rect[2] as f32, y + e.rect[3] as f32];
                    let view = GuiEntityView::new(rect, e.scale, 0.0, GuiEntityLook::At(mouse));
                    let _ =
                        draw.entity(&GuiEntity::with_data(e.kind.clone(), e.data.clone()), view);
                }
                if let Some(a) = display.arrow {
                    draw_arrow(draw, [origin[0] + a[0] as f32, origin[1] + a[1] as f32]);
                }
                for slot in &display.slots {
                    let r = Geometry::slot_rect(*origin, slot.pos);
                    draw.fill(r, SLOT_BG);
                    let Some(key) = cycled(&slot.items).and_then(|id| rt.key(id)) else {
                        continue;
                    };
                    draw.item(key, [r[0] + 1.0, r[1] + 1.0], 16.0);
                    if slot.count > 1 {
                        let n = slot.count.to_string();
                        let w = draw.text_width(&n);
                        draw.text(&n, [r[0] + 17.0 - w, r[1] + 9.0], BUTTON_TEXT, true);
                    }
                    if inside(r, mouse) {
                        draw.fill(r, HOVER);
                        tooltip = Some(key);
                    }
                }
                for line in &display.lines {
                    let text = tr(host, &line.key, &line.args);
                    draw.text(
                        &text,
                        [
                            origin[0] + line.pos[0] as f32,
                            origin[1] + line.pos[1] as f32,
                        ],
                        TEXT,
                        false,
                    );
                }
                if display.recipe_id.is_some() {
                    draw_button(draw, *plus, "+", inside(*plus, mouse));
                }
            }
        }

        if let Some(key) = tooltip {
            let name = host.hover_name(&ItemStack::of(key));
            let _ = event.draw.gui_tooltip(&[Text::literal(name)], mouse);
        }
    }

    /// A click on the recipe screen or the bookmarks. True when it was ours.
    fn mouse(
        &mut self,
        screen: &ScreenView<'_>,
        size: [f32; 2],
        button: i32,
        modifiers: u32,
    ) -> bool {
        let at = screen.mouse;
        let right = button == 1;
        let bookmark = self
            .bookmark_rects(screen, size)
            .iter()
            .position(|r| inside(*r, at))
            .and_then(|i| self.bookmarks.items().get(i).cloned());
        let hit = self.geometry(size).map(|(g, page)| {
            let displays: Vec<&Display> = page.iter().map(|(_, d)| *d).collect();
            let hit = g.hit(at, &displays);
            let target = match hit {
                Hit::Slot { recipe, slot } => page
                    .get(recipe)
                    .and_then(|(_, d)| d.slots.get(slot))
                    .and_then(|s| cycled(&s.items))
                    .map(str::to_owned),
                Hit::Plus(recipe) => page.get(recipe).and_then(|(_, d)| d.recipe_id.clone()),
                _ => None,
            };
            (hit, g.per_page, target)
        });
        match hit {
            Some((Hit::Outside, ..)) | None => {
                let Some(item) = bookmark else {
                    // While the recipe screen is up, the container under it takes no clicks;
                    // the item list beside it still does.
                    return hit.is_some() && inside(screen.rect, at);
                };
                self.click_item(&item, right);
                true
            }
            Some((hit, per_page, target)) => {
                let view = self.view.as_mut().expect("geometry needs a view");
                match hit {
                    Hit::Tab(t) => view.select(t),
                    Hit::PrevCategory => view.turn_tab(false),
                    Hit::NextCategory => view.turn_tab(true),
                    Hit::PrevPage => view.turn_page(false, per_page),
                    Hit::NextPage => view.turn_page(true, per_page),
                    Hit::Slot { .. } => {
                        if let Some(item) = target {
                            self.click_item(&item, right);
                        }
                    }
                    Hit::Plus(_) => {
                        if let Some(recipe) = target {
                            let max = ClientModifiers::new(modifiers).shift();
                            self.pending.push(Action::Transfer { recipe, max });
                            // JEI goes back to the container once the ingredients moved.
                            self.close();
                        }
                    }
                    Hit::Catalyst(_) => {
                        let cat = view.category();
                        self.open_category(cat);
                    }
                    Hit::Frame | Hit::Outside => {}
                }
                true
            }
        }
    }

    /// The item under the pointer on the recipe screen or the bookmarks, for R, U and A there.
    fn hovered(&self, screen: &ScreenView<'_>, size: [f32; 2]) -> Option<String> {
        let at = screen.mouse;
        if let Some((g, page)) = self.geometry(size) {
            let displays: Vec<&Display> = page.iter().map(|(_, d)| *d).collect();
            if let Hit::Slot { recipe, slot } = g.hit(at, &displays) {
                return cycled(&displays[recipe].slots[slot].items).map(str::to_owned);
            }
            if let Some(i) = g.catalysts.iter().position(|r| inside(*r, at)) {
                let rt = self.runtime.as_ref()?;
                return rt.index.catalysts[self.view.as_ref()?.category()]
                    .get(i)
                    .cloned();
            }
        }
        let i = self
            .bookmark_rects(screen, size)
            .iter()
            .position(|r| inside(*r, at))?;
        self.bookmarks.items().get(i).cloned()
    }
}

/// The size the screen hooks draw on: the GUI's, in GUI pixels. The key hook has no draw
/// context, so the last frame's size is kept.
static GUI_SIZE: Mutex<[f32; 2]> = Mutex::new([0.0, 0.0]);

fn gui_size() -> [f32; 2] {
    *GUI_SIZE.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for (id, key, label) in [
            (KEY_RECIPES, "key.keyboard.r", "key.jei.showRecipe"),
            (KEY_USES, "key.keyboard.u", "key.jei.showUses"),
            (KEY_BOOKMARK, "key.keyboard.a", "key.jei.bookmark"),
        ] {
            r.register_key_mapping(
                KeyRegistration::new(id, key)
                    .label(label)
                    .context(mod_api::KeyConflictContext::Gui),
            );
        }
        r.register_key_mapping(
            KeyRegistration::new(KEY_CHEAT, "key.keyboard.o")
                .label("key.jei.toggleCheatMode")
                .modifiers(ClientModifiers::new(2))
                .context(mod_api::KeyConflictContext::Gui),
        );
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut mod_api::ClientTickPostEvent) {
        let mut s = state();
        if s.runtime.is_none() || s.stale {
            if let Some(rt) = build(host) {
                s.runtime = Some(rt);
                s.stale = false;
                s.view = None;
                s.history.clear();
                s.refilter(host);
            }
        }
        if s.runtime.is_none() {
            return;
        }
        if !s.bookmarks_loaded {
            s.bookmarks_loaded = true;
            let path = ConfigPath::new(BOOKMARKS_FILE);
            if let Some(Ok(bytes)) = path.ok().map(|p| host.data_files().read(&p)) {
                s.bookmarks = Bookmarks::load(&String::from_utf8_lossy(&bytes));
            }
        }
        if s.bookmarks_dirty {
            s.bookmarks_dirty = false;
            if let Ok(path) = ConfigPath::new(BOOKMARKS_FILE) {
                let _ = host
                    .data_files()
                    .write(&path, s.bookmarks.save().as_bytes());
            }
        }
        if s.panel_dirty {
            s.panel_dirty = false;
            if !s.styled {
                s.styled = true;
                host.panel_style(MOD_ID, PanelStyle::Viewer);
            }
            let rt = s.runtime.as_ref().expect("checked above");
            let stacks = s
                .shown
                .iter()
                .map(|&i| Stack {
                    item: rt.items[i].key,
                    count: 1,
                    components: String::new(),
                })
                .collect();
            host.panel_items(MOD_ID, stacks);
        }
        for action in std::mem::take(&mut s.pending) {
            match action {
                Action::Transfer { recipe, max } => host.transfer_recipe_id(&recipe, max),
                // Refused outside creative: cheat mode then does nothing, as on a server
                // that doesn't let the player cheat.
                Action::Give { item, count } => {
                    let _ = host.give(item, count);
                }
            }
        }
    }

    fn recipes_received(&self, _: &mut dyn ClientHost, _: &mut RecipesReceivedEvent<'_>) {
        state().stale = true;
    }

    fn client_level_unload(
        &self,
        _: &mut dyn ClientHost,
        _: &mut mod_api::ClientLevelUnloadEvent<'_>,
    ) {
        let mut s = state();
        s.runtime = None;
        s.styled = false;
        s.bookmarks_loaded = false;
        s.close();
    }

    fn panel_event(&self, host: &mut dyn ClientHost, event: &mut PanelEventEvent<'_>) {
        if event.mod_id != MOD_ID {
            return;
        }
        let mut s = state();
        match event.event {
            PanelEvent::Clicked { index, right } => {
                let item = s
                    .shown
                    .get(*index as usize)
                    .and_then(|&i| s.runtime.as_ref().map(|rt| rt.items[i].entry.id.clone()));
                if let Some(item) = item {
                    s.click_item(&item, *right);
                }
            }
            PanelEvent::Search(text) => {
                s.query_text = text.clone();
                s.refilter(host);
            }
            PanelEvent::Key { key, stack } => {
                let item = stack.item.id(host);
                s.item_key(host, key, &item);
            }
            _ => {}
        }
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        *GUI_SIZE.lock().unwrap_or_else(|e| e.into_inner()) = event.draw.size();
        if !is_container(event.screen) {
            return;
        }
        state().draw(host, event);
    }

    fn screen_exclusion_areas(
        &self,
        _: &dyn ClientHost,
        event: &mut ScreenExclusionAreasEvent<'_>,
    ) {
        let s = state();
        if let Some(r) = view::bounds(&s.bookmark_rects(event.screen, gui_size())) {
            event.areas.push(r);
        }
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        _: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        if !is_container(event.screen) {
            return Flow::Continue;
        }
        if state().mouse(event.screen, gui_size(), event.button, event.modifiers) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn screen_mouse_scrolled_pre(
        &self,
        _: &dyn ClientHost,
        event: &mut ScreenMouseScrolledPreEvent<'_>,
    ) -> Flow {
        let mut s = state();
        let Some((g, _)) = s.geometry(gui_size()) else {
            return Flow::Continue;
        };
        if !inside(g.frame, event.screen.mouse) {
            return Flow::Continue;
        }
        let per_page = g.per_page;
        if let Some(v) = s.view.as_mut() {
            v.turn_page(event.scroll[1] < 0.0, per_page);
        }
        Flow::Cancel
    }

    fn screen_key_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenKeyPressedPreEvent<'_>,
    ) -> Flow {
        if !is_container(event.screen) {
            return Flow::Continue;
        }
        let mut s = state();
        let cheat = host.mod_key(MOD_ID, KEY_CHEAT);
        if cheat
            .as_ref()
            .is_some_and(|(k, m)| k == event.key && event.modifiers & 7 == m.bits & 7)
        {
            s.cheat = !s.cheat;
            return Flow::Cancel;
        }
        if s.view.is_some() {
            match event.key {
                "key.keyboard.escape" => {
                    s.close();
                    return Flow::Cancel;
                }
                "key.keyboard.backspace" => {
                    s.view = s.history.pop();
                    return Flow::Cancel;
                }
                _ => {}
            }
        }
        if let Some(item) = s.hovered(event.screen, gui_size()) {
            if s.item_key(host, event.key, &item) {
                return Flow::Cancel;
            }
        }
        Flow::Continue
    }

    fn screen_closing(&self, _: &dyn ClientHost, _: &mut ScreenClosingEvent<'_>) {
        state().close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translations_fill_their_arguments_in_order_and_by_position() {
        let args = ["10".to_owned(), "0.7".to_owned()];
        assert_eq!(fill("%ss, %s XP", &args), "10s, 0.7 XP");
        assert_eq!(fill("%2$s XP in %1$ss", &args), "0.7 XP in 10s");
        assert_eq!(fill("Chance: %s%%", &args[..1]), "Chance: 10%");
    }

    #[test]
    fn recipe_files_name_their_tags() {
        let json = mod_api::serde_json::json!({
            "type": "minecraft:crafting_shaped",
            "key": {"#": {"tag": "c:ingots/iron"}, "X": "#minecraft:planks", "S": "minecraft:stick"},
            "ingredients": [["#c:gems/diamond", "minecraft:emerald"]]
        });
        let mut tags = HashSet::new();
        tags_in(&json, &mut tags);
        let mut tags: Vec<_> = tags.into_iter().collect();
        tags.sort();
        assert_eq!(
            tags,
            ["c:gems/diamond", "c:ingots/iron", "minecraft:planks"]
        );
    }
}
