//! A book reader's data side for Horizon ports: what Patchouli, GuideME and Modonomicon show,
//! as one model a screen draws.
//!
//! - [`patchouli`]: Patchouli's public data format (`book.json`, `categories/`, `entries/`) read
//!   into a [`Book`]: categories, entries, typed pages, entry unlock conditions.
//! - [`markup`]: Patchouli's text markup (`$(br)`, `$(l:url)`, `$(thing)`, colours, `$(item)`)
//!   into styled spans, and [`markup::paginate`], which cuts text into pages of lines for a
//!   given width and height with the game's own text measure.
//!
//! Written from Patchouli's public documentation. GuideME (markdown pages) and Modonomicon
//! (its own JSON) are not read yet: they load into the same [`Book`] (README.md).
//! The screen that draws a book is the next piece; it needs the screens API's text and sprite
//! drawing and a mod-owned open path (an item use or a command).

pub mod markup;
pub mod patchouli;

use std::collections::BTreeMap;

/// A book: categories in order, entries by id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Book {
    pub id: String,
    pub name: String,
    pub landing_text: String,
    pub categories: Vec<Category>,
    pub entries: BTreeMap<String, Entry>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub description: String,
    /// An item id or a texture.
    pub icon: String,
    pub sortnum: i32,
    /// Another category this one sits inside.
    pub parent: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub category: String,
    pub icon: String,
    pub sortnum: i32,
    /// An advancement id that unlocks the entry; none means always shown.
    pub advancement: Option<String>,
    pub priority: bool,
    pub secret: bool,
    pub pages: Vec<Page>,
}

/// One page of an entry. Types a reader doesn't know stay as [`Page::Other`], so nothing a
/// mod ships is lost.
#[derive(Clone, Debug, PartialEq)]
pub enum Page {
    Text { title: Option<String>, text: String },
    Image { title: Option<String>, images: Vec<String>, border: bool, text: Option<String> },
    Crafting { recipe: String, second: Option<String>, title: Option<String>, text: Option<String> },
    Smelting { recipe: String, second: Option<String>, title: Option<String>, text: Option<String> },
    Spotlight { item: String, title: Option<String>, link_recipe: bool, text: Option<String> },
    Link { url: String, link_text: String, text: Option<String> },
    Relations { entries: Vec<String>, title: Option<String> },
    Empty,
    Other { kind: String, data: serde_json::Value },
}
