//! The ingredient list's search box, as JEI documents its syntax: words that must all match the
//! item's name, a prefix per word for another field (`@` mod, `#` tooltip, `$` tag, `&` resource
//! id, `%` creative tab, `^` colour), `-` before a word to exclude what it matches, `|` between
//! alternatives, and `"..."` to keep spaces inside one word. Matching ignores case.
//!
//! This module knows nothing of the game: an [`Entry`] carries what a word can match, and the
//! mod fills it from the host (`lib.rs`).

/// What one word of the query looks at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    /// The item's name in the client's language (no prefix).
    Name,
    /// `@`: the mod that adds it (its namespace, and the mod's name when known).
    Mod,
    /// `#`: any line of its tooltip.
    Tooltip,
    /// `$`: the name of a tag it is in.
    Tag,
    /// `&`: its resource id (`minecraft:oak_log`).
    ResourceId,
    /// `%`: the creative tab that lists it. Not yet: Horizon's mod API does not tell a mod which
    /// tab lists an item, so such a word matches everything.
    CreativeTab,
    /// `^`: its colour. Not yet (JEI reads the texture's colours), matches everything.
    Colour,
}

impl Field {
    fn of_prefix(c: char) -> Option<Self> {
        Some(match c {
            '@' => Field::Mod,
            '#' => Field::Tooltip,
            '$' => Field::Tag,
            '&' => Field::ResourceId,
            '%' => Field::CreativeTab,
            '^' => Field::Colour,
            _ => return None,
        })
    }
}

/// One word of the query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Term {
    pub field: Field,
    /// Lower-cased text the field must contain.
    pub text: String,
    /// `-word`: the item must not match.
    pub negate: bool,
}

/// A parsed query: any of `alternatives` (split at `|`), each needing all its terms.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    pub alternatives: Vec<Vec<Term>>,
}

/// What a query can look at for one item. `tooltip` is filled only when a query needs it
/// (building every item's tooltip is slow), see [`Query::needs`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    /// `minecraft:oak_log`.
    pub id: String,
    /// The name the client shows.
    pub name: String,
    /// The mod's display name, when known; the namespace always counts.
    pub mod_name: String,
    /// Tags it is in, as `minecraft:logs`.
    pub tags: Vec<String>,
    /// Its tooltip lines, once read.
    pub tooltip: Option<Vec<String>>,
}

impl Entry {
    /// An item with its id and name, nothing else known yet.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            ..Self::default()
        }
    }
    fn namespace(&self) -> &str {
        self.id.split_once(':').map_or("minecraft", |(ns, _)| ns)
    }
}

/// Split the text into words, keeping `"quoted words"` (with their prefix) whole.
fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for c in text.chars() {
        match c {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn term(word: &str) -> Option<Term> {
    let (negate, rest) = match word.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, word),
    };
    let mut chars = rest.chars();
    let (field, text) = match chars.next().and_then(Field::of_prefix) {
        Some(field) => (field, chars.as_str()),
        None => (Field::Name, rest),
    };
    let text = text.to_lowercase();
    // A bare prefix ("@", "-") is a word still being typed: it filters nothing.
    (!text.is_empty()).then_some(Term {
        field,
        text,
        negate,
    })
}

/// Parse what the player typed.
pub fn parse(text: &str) -> Query {
    let alternatives = text
        .split('|')
        .map(|alt| {
            words(alt)
                .iter()
                .filter_map(|w| term(w))
                .collect::<Vec<_>>()
        })
        .filter(|terms| !terms.is_empty())
        .collect();
    Query { alternatives }
}

fn contains(hay: &str, needle: &str) -> bool {
    hay.to_lowercase().contains(needle)
}

impl Term {
    fn hits(&self, e: &Entry) -> bool {
        let hit = match self.field {
            Field::Name => contains(&e.name, &self.text),
            Field::Mod => {
                contains(e.namespace(), &self.text)
                    || (!e.mod_name.is_empty() && contains(&e.mod_name, &self.text))
            }
            Field::Tooltip => e
                .tooltip
                .as_ref()
                .is_some_and(|lines| lines.iter().any(|l| contains(l, &self.text))),
            Field::Tag => e.tags.iter().any(|t| contains(t, &self.text)),
            Field::ResourceId => contains(&e.id, &self.text),
            Field::CreativeTab | Field::Colour => return true,
        };
        hit != self.negate
    }
}

impl Query {
    /// Whether the query filters nothing.
    pub fn is_empty(&self) -> bool {
        self.alternatives.is_empty()
    }
    /// Whether any word looks at `field`, so the caller fills it before matching.
    pub fn needs(&self, field: Field) -> bool {
        self.alternatives.iter().flatten().any(|t| t.field == field)
    }
    /// The words of a field, for the caller to look things up once (the tags they name).
    pub fn texts(&self, field: Field) -> impl Iterator<Item = &str> {
        self.alternatives
            .iter()
            .flatten()
            .filter(move |t| t.field == field)
            .map(|t| t.text.as_str())
    }
    /// Whether `e` passes.
    pub fn matches(&self, e: &Entry) -> bool {
        self.is_empty()
            || self
                .alternatives
                .iter()
                .any(|terms| terms.iter().all(|t| t.hits(e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log() -> Entry {
        Entry {
            id: "minecraft:oak_log".into(),
            name: "Oak Log".into(),
            mod_name: "Minecraft".into(),
            tags: vec!["minecraft:logs".into(), "minecraft:oak_logs".into()],
            tooltip: Some(vec!["Oak Log".into(), "minecraft:oak_log".into()]),
        }
    }

    fn ingot() -> Entry {
        Entry {
            id: "mekanism:ingot_osmium".into(),
            name: "Osmium Ingot".into(),
            mod_name: "Mekanism".into(),
            tags: vec!["c:ingots/osmium".into()],
            tooltip: Some(vec!["Osmium Ingot".into(), "Mekanism".into()]),
        }
    }

    #[test]
    fn plain_words_must_all_be_in_the_name_whatever_the_case() {
        assert!(parse("oak LOG").matches(&log()));
        assert!(parse("log oak").matches(&log()));
        assert!(!parse("oak plank").matches(&log()));
    }

    #[test]
    fn prefixes_pick_mod_tag_tooltip_and_id() {
        assert!(parse("@mek").matches(&ingot()));
        assert!(!parse("@mek").matches(&log()));
        // The namespace counts even where the mod's name doesn't contain it.
        assert!(parse("@minecraft").matches(&log()));
        assert!(parse("$ingots").matches(&ingot()));
        assert!(!parse("$ingots").matches(&log()));
        assert!(parse("&oak_log").matches(&log()));
        assert!(parse("#mekanism").matches(&ingot()));
    }

    #[test]
    fn a_minus_excludes_and_a_bar_offers_alternatives() {
        assert!(!parse("-@minecraft").matches(&log()));
        assert!(parse("-@minecraft").matches(&ingot()));
        assert!(parse("ingot -oak").matches(&ingot()));
        let either = parse("osmium|oak");
        assert!(either.matches(&log()) && either.matches(&ingot()));
        assert!(!parse("osmium|diamond").matches(&log()));
    }

    #[test]
    fn quotes_keep_a_space_inside_one_word() {
        assert!(parse("\"oak log\"").matches(&log()));
        assert!(!parse("\"log oak\"").matches(&log()));
    }

    #[test]
    fn a_word_still_being_typed_filters_nothing() {
        assert!(parse("@").is_empty());
        assert!(parse(" - ").is_empty());
        assert!(parse("").matches(&ingot()));
    }

    #[test]
    fn a_tooltip_word_asks_for_tooltips_and_misses_without_them() {
        let q = parse("#osmium");
        assert!(q.needs(Field::Tooltip));
        assert!(!parse("osmium").needs(Field::Tooltip));
        let mut e = ingot();
        e.tooltip = None;
        assert!(!q.matches(&e));
    }
}
