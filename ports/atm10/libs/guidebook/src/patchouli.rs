//! Patchouli's data files into a [`Book`]. The layout is `data/<ns>/patchouli_books/<book>/`:
//! `book.json`, and per language `en_us/categories/*.json` and `en_us/entries/**/*.json`.

use serde::Deserialize;
use serde_json::Value;

use crate::{Book, Category, Entry, Page};

#[derive(Debug)]
pub enum Error {
    Json { file: String, error: serde_json::Error },
    /// An entry names a category the book doesn't have.
    UnknownCategory { entry: String, category: String },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Json { file, error } => write!(f, "patchouli: {file}: {error}"),
            Error::UnknownCategory { entry, category } => write!(f, "patchouli: entry {entry} is in unknown category {category}"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Deserialize)]
struct RawBook {
    name: String,
    #[serde(default)]
    landing_text: String,
}

#[derive(Deserialize)]
struct RawCategory {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    icon: String,
    #[serde(default)]
    sortnum: i32,
    parent: Option<String>,
}

#[derive(Deserialize)]
struct RawEntry {
    name: String,
    category: String,
    #[serde(default)]
    icon: String,
    #[serde(default)]
    sortnum: i32,
    advancement: Option<String>,
    #[serde(default)]
    priority: bool,
    #[serde(default, rename = "secret")]
    secret: bool,
    #[serde(default)]
    pages: Vec<Value>,
}

fn json<T: for<'a> Deserialize<'a>>(file: &str, bytes: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(|error| Error::Json { file: file.to_owned(), error })
}

/// A book from its files: `(path inside the language folder, bytes)`, e.g.
/// `("categories/basics.json", ..)` and `("entries/basics/start.json", ..)`. `book_json` is the
/// book's own `book.json`. A category or entry id is its path without folder prefix and `.json`
/// (`basics`, `basics/start`), namespaced with `namespace` (`mymod:basics`).
pub fn read_book(namespace: &str, id: &str, book_json: &[u8], files: &[(String, Vec<u8>)]) -> Result<Book, Error> {
    let raw: RawBook = json("book.json", book_json)?;
    let mut book = Book { id: format!("{namespace}:{id}"), name: raw.name, landing_text: raw.landing_text, ..Book::default() };
    for (path, bytes) in files {
        if let Some(name) = path.strip_prefix("categories/").and_then(|p| p.strip_suffix(".json")) {
            let c: RawCategory = json(path, bytes)?;
            book.categories.push(Category {
                id: format!("{namespace}:{name}"),
                name: c.name,
                description: c.description,
                icon: c.icon,
                sortnum: c.sortnum,
                parent: c.parent,
            });
        }
    }
    book.categories.sort_by(|a, b| a.sortnum.cmp(&b.sortnum).then_with(|| a.id.cmp(&b.id)));
    for (path, bytes) in files {
        if let Some(name) = path.strip_prefix("entries/").and_then(|p| p.strip_suffix(".json")) {
            let e: RawEntry = json(path, bytes)?;
            let category = if e.category.contains(':') { e.category } else { format!("{namespace}:{}", e.category) };
            if !book.categories.iter().any(|c| c.id == category) {
                return Err(Error::UnknownCategory { entry: name.to_owned(), category });
            }
            let id = format!("{namespace}:{name}");
            book.entries.insert(
                id.clone(),
                Entry {
                    id,
                    name: e.name,
                    category,
                    icon: e.icon,
                    sortnum: e.sortnum,
                    advancement: e.advancement,
                    priority: e.priority,
                    secret: e.secret,
                    pages: e.pages.into_iter().map(page).collect(),
                },
            );
        }
    }
    Ok(book)
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_owned)
}

fn page(v: Value) -> Page {
    // A page may be a bare string: a text page.
    if let Some(text) = v.as_str() {
        return Page::Text { title: None, text: text.to_owned() };
    }
    let kind = s(&v, "type").unwrap_or_default();
    let kind = kind.strip_prefix("patchouli:").unwrap_or(&kind).to_owned();
    match kind.as_str() {
        "text" => Page::Text { title: s(&v, "title"), text: s(&v, "text").unwrap_or_default() },
        "image" => Page::Image {
            title: s(&v, "title"),
            images: v.get("images").and_then(Value::as_array).map(|a| a.iter().filter_map(|i| i.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
            border: v.get("border").and_then(Value::as_bool).unwrap_or(false),
            text: s(&v, "text"),
        },
        "crafting" => Page::Crafting { recipe: s(&v, "recipe").unwrap_or_default(), second: s(&v, "recipe2"), title: s(&v, "title"), text: s(&v, "text") },
        "smelting" => Page::Smelting { recipe: s(&v, "recipe").unwrap_or_default(), second: s(&v, "recipe2"), title: s(&v, "title"), text: s(&v, "text") },
        "spotlight" => Page::Spotlight {
            item: s(&v, "item").unwrap_or_default(),
            title: s(&v, "title"),
            link_recipe: v.get("link_recipe").and_then(Value::as_bool).unwrap_or(false),
            text: s(&v, "text"),
        },
        "link" => Page::Link { url: s(&v, "url").unwrap_or_default(), link_text: s(&v, "link_text").unwrap_or_default(), text: s(&v, "text") },
        "relations" => Page::Relations {
            entries: v.get("entries").and_then(Value::as_array).map(|a| a.iter().filter_map(|i| i.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
            title: s(&v, "title"),
        },
        "empty" => Page::Empty,
        _ => Page::Other { kind, data: v },
    }
}

impl Book {
    /// The entries of `category`, priority ones first, then by sort number and name.
    pub fn entries_in(&self, category: &str) -> Vec<&Entry> {
        let mut list: Vec<&Entry> = self.entries.values().filter(|e| e.category == category).collect();
        list.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.sortnum.cmp(&b.sortnum)).then_with(|| a.name.cmp(&b.name)));
        list
    }

    /// The entries a reader shows: those with no advancement, or one `has` says is done; secret
    /// ones only when unlocked.
    pub fn visible<'a>(&'a self, category: &str, has: &dyn Fn(&str) -> bool) -> Vec<&'a Entry> {
        self.entries_in(category)
            .into_iter()
            .filter(|e| match &e.advancement {
                Some(a) => has(a),
                None => !e.secret,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files() -> Vec<(String, Vec<u8>)> {
        let f = |p: &str, j: &str| (p.to_owned(), j.as_bytes().to_vec());
        vec![
            f("categories/basics.json", r#"{"name": "Basics", "icon": "minecraft:book", "sortnum": 1}"#),
            f("categories/machines.json", r#"{"name": "Machines", "sortnum": 0, "description": "Big ones"}"#),
            f(
                "entries/basics/start.json",
                r#"{"name": "Start", "category": "basics", "icon": "minecraft:stick", "priority": true, "pages": [
                    "Hello $(br)world",
                    {"type": "patchouli:crafting", "recipe": "mymod:rod", "text": "Make it"},
                    {"type": "patchouli:image", "images": ["mymod:textures/a.png"], "border": true},
                    {"type": "mymod:custom", "x": 1}]}"#,
            ),
            f("entries/basics/later.json", r#"{"name": "Later", "category": "mymod:basics", "advancement": "mymod:done", "sortnum": 2}"#),
            f("entries/basics/hidden.json", r#"{"name": "Hidden", "category": "basics", "secret": true}"#),
        ]
    }

    #[test]
    fn a_book_reads_in_order_with_typed_pages() {
        let b = read_book("mymod", "guide", br#"{"name": "Guide", "landing_text": "Welcome"}"#, &files()).unwrap();
        assert_eq!(b.id, "mymod:guide");
        assert_eq!(b.categories.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Machines", "Basics"]);
        let start = &b.entries["mymod:basics/start"];
        assert_eq!(start.pages.len(), 4);
        assert_eq!(start.pages[0], Page::Text { title: None, text: "Hello $(br)world".into() });
        assert!(matches!(&start.pages[1], Page::Crafting { recipe, .. } if recipe == "mymod:rod"));
        assert!(matches!(&start.pages[2], Page::Image { border: true, .. }));
        assert!(matches!(&start.pages[3], Page::Other { kind, .. } if kind == "mymod:custom"));
    }

    #[test]
    fn locked_and_secret_entries_wait_for_their_advancement() {
        let b = read_book("mymod", "guide", br#"{"name": "Guide"}"#, &files()).unwrap();
        let names = |has: &dyn Fn(&str) -> bool| b.visible("mymod:basics", has).iter().map(|e| e.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&|_| false), ["Start"]);
        assert_eq!(names(&|a| a == "mymod:done"), ["Start", "Later"]);
    }

    #[test]
    fn an_entry_in_an_unknown_category_is_an_error() {
        let mut f = files();
        f.push(("entries/x.json".into(), br#"{"name": "X", "category": "nope"}"#.to_vec()));
        assert!(matches!(read_book("mymod", "g", br#"{"name": "G"}"#, &f), Err(Error::UnknownCategory { .. })));
    }
}
