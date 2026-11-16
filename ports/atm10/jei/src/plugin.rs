//! JEI's plugin interface for other Horizon ports: a mod adds recipe categories, recipes and
//! their catalysts (the blocks that make them) by implementing [`JeiPlugin`] and handing it to
//! [`register_plugin`] from its own `register` hook.
//!
//! It follows the shape of JEI's Java API (`IModPlugin.registerCategories`,
//! `registerRecipes`, `registerRecipeCatalysts`, `IRecipeLayoutBuilder`), written for Rust and
//! for what a Horizon viewer draws. A recipe here is plain data: the slots it shows, where they
//! sit, and the lines of text beside them; JEI draws, pages and searches them.
//!
//! A port that uses it declares JEI in its `horizon-mod.toml` (`[dependencies] jei = "^19"`),
//! so the launcher links the two and runs JEI's hooks first; the plugin is asked again each time
//! JEI rebuilds its recipes (joining a world, recipes arriving from a server).
use std::sync::{Arc, Mutex};

use mod_api::{Compound, Host};

/// A recipe category: a tab of the recipe screen (`IRecipeCategory`).
#[derive(Clone, Debug, PartialEq)]
pub struct Category {
    /// Unique id, by convention the recipe type it shows (`minecraft:smelting`,
    /// `justenoughbreeding:breeding`).
    pub uid: String,
    /// Translation key of the tab's title.
    pub title: String,
    /// Item whose icon the tab shows (`minecraft:furnace`).
    pub icon: String,
    /// Size of one recipe's drawing, GUI pixels.
    pub width: u16,
    pub height: u16,
}

impl Category {
    pub fn new(uid: &str, title: &str, icon: &str, width: u16, height: u16) -> Self {
        Self {
            uid: uid.into(),
            title: title.into(),
            icon: icon.into(),
            width,
            height,
        }
    }
}

/// What a slot is to the recipe, which decides whether it answers "R" or "U" for its items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Consumed: the recipe is a use of these items (`RecipeIngredientRole.INPUT`).
    Input,
    /// Made: the recipe is a way to get these items (`OUTPUT`).
    Output,
    /// Needed but not consumed, drawn like an input (a breeding animal, a workstation):
    /// `CATALYST` in JEI's roles. Counts as a use.
    Catalyst,
    /// Only drawn; no lookup finds the recipe through it (`RENDER_ONLY`).
    RenderOnly,
}

/// One slot: what it may show (the alternatives cycle, once a second), at `pos` inside the
/// recipe's drawing.
#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    pub role: Role,
    /// GUI pixels from the recipe's top left corner to the slot's 16×16 item.
    pub pos: [i16; 2],
    /// Item ids, in the order they cycle.
    pub items: Vec<String>,
    /// Count drawn on the item, 1 for none.
    pub count: u32,
}

/// A line of text in a recipe: a translation key and its arguments, already as text.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub pos: [i16; 2],
    pub key: String,
    pub args: Vec<String>,
}

/// A living entity drawn in a recipe (a breeding animal, a villager), as JEI plugins draw
/// one with `InventoryScreen.renderEntityInInventory`.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityShown {
    /// Its type (`minecraft:cow`).
    pub kind: String,
    /// Saved fields it is made from, as `/summon` reads them (`{VillagerData:{...}}`).
    pub data: Compound,
    /// The box it stands in, `[x, y, width, height]` inside the recipe's drawing.
    pub rect: [i16; 4],
    /// GUI pixels per block.
    pub scale: f32,
}

/// One recipe as JEI shows it (`IRecipeLayoutBuilder`'s result).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Display {
    /// The recipe's id when it has one the server knows, for "+" (`transfer_recipe_id`).
    pub recipe_id: Option<String>,
    pub slots: Vec<Slot>,
    pub lines: Vec<Line>,
    /// Draw the furnace-style arrow at this point.
    pub arrow: Option<[i16; 2]>,
    /// An entity drawn in the recipe.
    pub entity: Option<EntityShown>,
}

impl Display {
    pub fn new(recipe_id: Option<String>) -> Self {
        Self {
            recipe_id,
            ..Self::default()
        }
    }
    /// Add a slot of `role` at `pos` showing `items`.
    pub fn slot(mut self, role: Role, pos: [i16; 2], items: Vec<String>) -> Self {
        self.slots.push(Slot {
            role,
            pos,
            items,
            count: 1,
        });
        self
    }
    /// Add a slot with a count.
    pub fn slot_counted(
        mut self,
        role: Role,
        pos: [i16; 2],
        items: Vec<String>,
        count: u32,
    ) -> Self {
        self.slots.push(Slot {
            role,
            pos,
            items,
            count,
        });
        self
    }
    pub fn input(self, pos: [i16; 2], items: Vec<String>) -> Self {
        self.slot(Role::Input, pos, items)
    }
    pub fn output(self, pos: [i16; 2], items: Vec<String>) -> Self {
        self.slot(Role::Output, pos, items)
    }
    /// Add a line of translated text.
    pub fn line(mut self, pos: [i16; 2], key: &str, args: Vec<String>) -> Self {
        self.lines.push(Line {
            pos,
            key: key.into(),
            args,
        });
        self
    }
    /// Draw an entity of `kind` made from `data` in `rect`, `scale` GUI pixels per block.
    pub fn entity(mut self, kind: &str, data: Compound, rect: [i16; 4], scale: f32) -> Self {
        self.entity = Some(EntityShown {
            kind: kind.into(),
            data,
            rect,
            scale,
        });
        self
    }
    pub fn arrow(mut self, pos: [i16; 2]) -> Self {
        self.arrow = Some(pos);
        self
    }
    /// The items of every slot of `role`.
    pub fn items_of(&self, role: Role) -> impl Iterator<Item = &str> {
        self.slots
            .iter()
            .filter(move |s| s.role == role)
            .flat_map(|s| s.items.iter().map(String::as_str))
    }
}

/// What plugins hand JEI while it builds its recipes.
#[derive(Default, Debug)]
pub struct Registration {
    pub(crate) categories: Vec<Category>,
    pub(crate) recipes: Vec<(String, Display)>,
    pub(crate) catalysts: Vec<(String, String)>,
    pub(crate) hidden: Vec<String>,
}

impl Registration {
    /// Add a category (`registerCategories`). A second category with the same uid is ignored.
    pub fn add_category(&mut self, category: Category) {
        if !self.categories.iter().any(|c| c.uid == category.uid) {
            self.categories.push(category);
        }
    }
    /// Add a recipe to category `uid` (`registerRecipes`). One of an unknown category is
    /// dropped when JEI builds its index.
    pub fn add_recipe(&mut self, uid: &str, display: Display) {
        self.recipes.push((uid.into(), display));
    }
    /// Item `item` makes category `uid`'s recipes (`registerRecipeCatalysts`): it is listed
    /// beside them, and "U" on it shows the whole category.
    pub fn add_catalyst(&mut self, uid: &str, item: &str) {
        self.catalysts.push((uid.into(), item.into()));
    }
    /// Keep `item` out of the ingredient list (`IIngredientManager.removeIngredientsAtRuntime`).
    pub fn hide(&mut self, item: &str) {
        self.hidden.push(item.into());
    }
    /// The categories added so far, for a plugin that adds to another's.
    pub fn categories(&self) -> &[Category] {
        &self.categories
    }
}

/// A mod's JEI plugin (`IModPlugin`). Every method has a default, so a plugin implements what
/// it adds. JEI calls them in this order, every plugin's first method before any second.
pub trait JeiPlugin: Send + Sync {
    /// The plugin's id (`IModPlugin.getPluginUid`), `<mod id>:<name>`.
    fn uid(&self) -> &str;
    /// Add categories.
    fn register_categories(&self, _host: &dyn Host, _r: &mut Registration) {}
    /// Add recipes; `host` reads recipes, tags and items.
    fn register_recipes(&self, _host: &dyn Host, _r: &mut Registration) {}
    /// Add catalysts.
    fn register_catalysts(&self, _host: &dyn Host, _r: &mut Registration) {}
}

static PLUGINS: Mutex<Vec<Arc<dyn JeiPlugin>>> = Mutex::new(Vec::new());

/// Give JEI a plugin. Call it from your mod's `register` hook; a plugin with a uid already
/// given replaces the earlier one.
pub fn register_plugin(plugin: Arc<dyn JeiPlugin>) {
    let mut plugins = PLUGINS.lock().unwrap_or_else(|e| e.into_inner());
    plugins.retain(|p| p.uid() != plugin.uid());
    plugins.push(plugin);
}

/// The plugins given so far, in order.
pub(crate) fn plugins() -> Vec<Arc<dyn JeiPlugin>> {
    PLUGINS.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Run every plugin's registration in JEI's order.
pub(crate) fn collect(host: &dyn Host, plugins: &[Arc<dyn JeiPlugin>]) -> Registration {
    let mut r = Registration::default();
    for p in plugins {
        p.register_categories(host, &mut r);
    }
    for p in plugins {
        p.register_recipes(host, &mut r);
    }
    for p in plugins {
        p.register_catalysts(host, &mut r);
    }
    r
}
