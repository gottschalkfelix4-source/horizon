//! Typed config files for ported mods, the way NeoForge's `ModConfigSpec` keeps them: a schema of
//! sections and values with defaults, ranges and allowed values, a TOML file per kind
//! (`config/<mod id>-common.toml`, `-client.toml`, a world's `serverconfig/<mod id>-server.toml`),
//! and, on load, every missing or invalid value put back to its default and the file written again.
//!
//! This is the config-library row of docs/modpacks.md: Horizon already reads and watches the
//! files (`Hooks::register_configs`, `config_loading`, `config_reloading`); this crate is the
//! schema on top, so a port declares its options once and reads them typed:
//!
//! ```
//! use atm10_config::{Spec, Store};
//! let spec = Spec::builder()
//!     .comment("Mega Torch settings")
//!     .push("megatorch")
//!     .comment("Radius in blocks along each axis")
//!     .define_int("radius", 64, 1, 512)
//!     .define_bool("block_village_sieges", true)
//!     .pop()
//!     .build();
//! let store = Store::new(spec);
//! let loaded = store.load("[megatorch]\nradius = 9000\n");
//! assert_eq!(store.int("megatorch.radius"), 64); // out of range: the default
//! assert!(loaded.corrected.is_some()); // the text to write back
//! ```
//!
//! Written from NeoForge's public documentation of config files, not from any mod's code.

use std::collections::BTreeMap;
use std::sync::RwLock;

use mod_api::{Config, ConfigFile, ConfigKind};

/// One value of a config file.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Double(f64),
    Str(String),
    List(Vec<Value>),
}

impl Value {
    fn from_toml(v: &toml::Value) -> Option<Value> {
        Some(match v {
            toml::Value::Boolean(b) => Value::Bool(*b),
            toml::Value::Integer(i) => Value::Int(*i),
            toml::Value::Float(f) => Value::Double(*f),
            toml::Value::String(s) => Value::Str(s.clone()),
            toml::Value::Array(a) => Value::List(a.iter().map(Value::from_toml).collect::<Option<_>>()?),
            _ => return None,
        })
    }

    fn write(&self, out: &mut String) {
        match self {
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Int(i) => out.push_str(&i.to_string()),
            Value::Double(d) => out.push_str(&format_double(*d)),
            Value::Str(s) => out.push_str(&toml::Value::String(s.clone()).to_string()),
            Value::List(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    item.write(out);
                }
                out.push(']');
            }
        }
    }
}

/// A double as the file shows it: always with a decimal point, as NightConfig writes one.
fn format_double(d: f64) -> String {
    let s = d.to_string();
    if s.contains(['.', 'e', 'E']) || !d.is_finite() {
        s
    } else {
        format!("{s}.0")
    }
}

/// What a value may hold.
#[derive(Clone, Debug, PartialEq)]
pub enum Rule {
    Bool,
    /// An integer in `min..=max` (`defineInRange` with ints or longs).
    Int { min: i64, max: i64 },
    /// A double in `min..=max`; an integer in the file is read as a double, as NightConfig does.
    Double { min: f64, max: f64 },
    /// Any string.
    Str,
    /// One of `allowed`, compared as written (`defineEnum`).
    Enum { allowed: Vec<String> },
    /// A list whose every element satisfies `element` (`defineList`); `allow_empty` false refuses
    /// an empty list (`defineListAllowEmpty` is true).
    List { element: Box<Rule>, allow_empty: bool },
}

impl Rule {
    /// The value as this rule keeps it, or None when it breaks the rule.
    fn check(&self, v: Value) -> Option<Value> {
        match (self, v) {
            (Rule::Bool, v @ Value::Bool(_)) => Some(v),
            (Rule::Int { min, max }, Value::Int(i)) => (*min..=*max).contains(&i).then_some(Value::Int(i)),
            (Rule::Double { min, max }, Value::Double(d)) => (d >= *min && d <= *max).then_some(Value::Double(d)),
            (Rule::Double { min, max }, Value::Int(i)) => {
                let d = i as f64;
                (d >= *min && d <= *max).then_some(Value::Double(d))
            }
            (Rule::Str, v @ Value::Str(_)) => Some(v),
            (Rule::Enum { allowed }, Value::Str(s)) => allowed.contains(&s).then_some(Value::Str(s)),
            (Rule::List { element, allow_empty }, Value::List(items)) => {
                if items.is_empty() && !allow_empty {
                    return None;
                }
                items.into_iter().map(|i| element.check(i)).collect::<Option<Vec<_>>>().map(Value::List)
            }
            _ => None,
        }
    }

    /// The comment line NeoForge adds under a value's own comment.
    fn hint(&self) -> Option<String> {
        match self {
            Rule::Int { min, max } if (*min, *max) != (i64::MIN, i64::MAX) => {
                Some(format!("Range: {}", range_text(&min.to_string(), &max.to_string(), *min == i64::MIN, *max == i64::MAX)))
            }
            Rule::Double { min, max } if (*min, *max) != (f64::MIN, f64::MAX) => Some(format!(
                "Range: {}",
                range_text(&format_double(*min), &format_double(*max), *min == f64::MIN, *max == f64::MAX)
            )),
            Rule::Enum { allowed } => Some(format!("Allowed Values: {}", allowed.join(", "))),
            _ => None,
        }
    }
}

fn range_text(min: &str, max: &str, open_min: bool, open_max: bool) -> String {
    match (open_min, open_max) {
        (true, false) => format!("< {max}"),
        (false, true) => format!("> {min}"),
        _ => format!("{min} ~ {max}"),
    }
}

/// One entry: a section's comment or a value.
#[derive(Clone, Debug)]
struct Entry {
    /// Dotted path: `megatorch.radius`.
    path: Vec<String>,
    comment: Vec<String>,
    /// None for a section.
    value: Option<(Rule, Value)>,
}

/// A config file's schema, built once with [`Spec::builder`].
#[derive(Clone, Debug, Default)]
pub struct Spec {
    entries: Vec<Entry>,
}

/// Builds a [`Spec`]: `comment` applies to the next `push` or `define_*`.
#[derive(Debug, Default)]
pub struct SpecBuilder {
    spec: Spec,
    section: Vec<String>,
    comment: Vec<String>,
}

impl SpecBuilder {
    /// A comment for the next section or value; each call adds one line, and a line with `\n`
    /// is split.
    pub fn comment(mut self, text: &str) -> Self {
        self.comment.extend(text.lines().map(str::to_owned));
        self
    }

    /// Opens section `name` inside the current one (`[a.b]`).
    pub fn push(mut self, name: &str) -> Self {
        self.section.push(name.to_owned());
        let comment = std::mem::take(&mut self.comment);
        self.spec.entries.push(Entry { path: self.section.clone(), comment, value: None });
        self
    }

    /// Closes the current section.
    pub fn pop(mut self) -> Self {
        self.section.pop();
        self
    }

    /// A value under `rule` with `default`. Panics when the default breaks the rule: that is a
    /// mistake in the port, found the first time it runs.
    pub fn define(mut self, name: &str, default: Value, rule: Rule) -> Self {
        let default = rule
            .check(default.clone())
            .unwrap_or_else(|| panic!("config default for {name} breaks its rule: {default:?}"));
        let mut path = self.section.clone();
        path.push(name.to_owned());
        let comment = std::mem::take(&mut self.comment);
        self.spec.entries.push(Entry { path, comment, value: Some((rule, default)) });
        self
    }

    pub fn define_bool(self, name: &str, default: bool) -> Self {
        self.define(name, Value::Bool(default), Rule::Bool)
    }

    pub fn define_int(self, name: &str, default: i64, min: i64, max: i64) -> Self {
        self.define(name, Value::Int(default), Rule::Int { min, max })
    }

    pub fn define_double(self, name: &str, default: f64, min: f64, max: f64) -> Self {
        self.define(name, Value::Double(default), Rule::Double { min, max })
    }

    pub fn define_str(self, name: &str, default: &str) -> Self {
        self.define(name, Value::Str(default.to_owned()), Rule::Str)
    }

    pub fn define_enum(self, name: &str, default: &str, allowed: &[&str]) -> Self {
        let allowed = allowed.iter().map(|s| (*s).to_owned()).collect();
        self.define(name, Value::Str(default.to_owned()), Rule::Enum { allowed })
    }

    /// A list of strings, any or empty allowed (`defineListAllowEmpty`).
    pub fn define_str_list(self, name: &str, default: &[&str]) -> Self {
        let items = default.iter().map(|s| Value::Str((*s).to_owned())).collect();
        self.define(name, Value::List(items), Rule::List { element: Box::new(Rule::Str), allow_empty: true })
    }

    pub fn build(self) -> Spec {
        self.spec
    }
}

impl Spec {
    pub fn builder() -> SpecBuilder {
        SpecBuilder::default()
    }

    /// The defaults, by dotted path.
    pub fn defaults(&self) -> BTreeMap<String, Value> {
        self.values().map(|(e, (_, d))| (e.path.join("."), d.clone())).collect()
    }

    fn values(&self) -> impl Iterator<Item = (&Entry, &(Rule, Value))> {
        self.entries.iter().filter_map(|e| e.value.as_ref().map(|v| (e, v)))
    }

    /// The file's text for `values` (the defaults where one is missing): top-level values
    /// first, then each section under its comment, values indented by a tab as NightConfig writes
    /// them.
    pub fn write(&self, values: &BTreeMap<String, Value>) -> String {
        let mut out = String::new();
        let mut sections: Vec<&Entry> = self.entries.iter().filter(|e| e.value.is_none()).collect();
        sections.sort_by(|a, b| a.path.cmp(&b.path));
        self.write_values(&[], values, &mut out);
        for section in sections {
            if !out.is_empty() {
                out.push('\n');
            }
            for line in &section.comment {
                out.push('#');
                out.push_str(line);
                out.push('\n');
            }
            out.push('[');
            out.push_str(&section.path.iter().map(|p| toml_key(p)).collect::<Vec<_>>().join("."));
            out.push_str("]\n");
            self.write_values(&section.path, values, &mut out);
        }
        out
    }

    fn write_values(&self, section: &[String], values: &BTreeMap<String, Value>, out: &mut String) {
        let indent = if section.is_empty() { "" } else { "\t" };
        for (entry, (rule, default)) in self.values() {
            let (name, parent) = entry.path.split_last().expect("a value has a name");
            if parent != section {
                continue;
            }
            for line in entry.comment.iter().cloned().chain(rule.hint()) {
                out.push_str(indent);
                out.push('#');
                out.push_str(&line);
                out.push('\n');
            }
            out.push_str(indent);
            out.push_str(&toml_key(name));
            out.push_str(" = ");
            values.get(&entry.path.join(".")).unwrap_or(default).write(out);
            out.push('\n');
        }
    }

    /// Reads `text`: each value the file holds that keeps its rule, else the default. `corrected`
    /// is the text to write back when anything was missing, invalid or unknown (NeoForge corrects
    /// the file the same way when it loads); an unreadable file is replaced by the defaults.
    pub fn read(&self, text: &str) -> Loaded {
        let table: toml::Table = match text.parse() {
            Ok(t) => t,
            Err(_) => {
                let values = self.defaults();
                let corrected = Some(self.write(&values));
                return Loaded { values, corrected, fixed: vec!["<whole file>".into()] };
            }
        };
        let mut values = BTreeMap::new();
        let mut fixed = Vec::new();
        for (entry, (rule, default)) in self.values() {
            let key = entry.path.join(".");
            let found = lookup(&table, &entry.path).and_then(Value::from_toml).and_then(|v| rule.check(v));
            match found {
                Some(v) => {
                    values.insert(key, v);
                }
                None => {
                    values.insert(key.clone(), default.clone());
                    fixed.push(key);
                }
            }
        }
        let extra = has_unknown(&table, &[], self);
        let corrected = (!fixed.is_empty() || extra).then(|| self.write(&values));
        Loaded { values, corrected, fixed }
    }

    fn knows(&self, path: &[String]) -> bool {
        self.entries.iter().any(|e| e.path == path)
    }

    /// The [`Config`] to push in `register_configs`: `config/<mod id>-<kind>.toml`, written with
    /// the defaults when missing.
    pub fn registration(&self, mod_id: &str, kind: ConfigKind) -> Config {
        Config::new(mod_id, kind, &self.write(&self.defaults()))
    }
}

fn toml_key(k: &str) -> String {
    if !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        k.to_owned()
    } else {
        toml::Value::String(k.to_owned()).to_string()
    }
}

fn lookup<'a>(table: &'a toml::Table, path: &[String]) -> Option<&'a toml::Value> {
    let (last, parents) = path.split_last()?;
    let mut t = table;
    for p in parents {
        t = t.get(p)?.as_table()?;
    }
    t.get(last)
}

fn has_unknown(table: &toml::Table, at: &[String], spec: &Spec) -> bool {
    table.iter().any(|(k, v)| {
        let mut path = at.to_vec();
        path.push(k.clone());
        if !spec.knows(&path) {
            return true;
        }
        matches!(v, toml::Value::Table(t) if has_unknown(t, &path, spec))
    })
}

/// What [`Spec::read`] made of a file.
#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    /// Every value, by dotted path.
    pub values: BTreeMap<String, Value>,
    /// The text to write back, when the file needed correcting.
    pub corrected: Option<String>,
    /// The paths put back to their default.
    pub fixed: Vec<String>,
}

/// A spec and its current values, readable from any hook: a port keeps one per config file in a
/// `static` (`LazyLock<Store>`), loads it in `config_loading` and `config_reloading`, and reads
/// it anywhere. Before the file loads, reads give the defaults.
#[derive(Debug)]
pub struct Store {
    spec: Spec,
    values: RwLock<BTreeMap<String, Value>>,
}

impl Store {
    pub fn new(spec: Spec) -> Self {
        let values = RwLock::new(spec.defaults());
        Self { spec, values }
    }

    pub fn spec(&self) -> &Spec {
        &self.spec
    }

    /// Takes the values of `text`; see [`Spec::read`].
    pub fn load(&self, text: &str) -> Loaded {
        let loaded = self.spec.read(text);
        *self.values.write().unwrap_or_else(|e| e.into_inner()) = loaded.values.clone();
        loaded
    }

    /// For `config_loading` and `config_reloading`: loads `config` when it is this mod's file
    /// `file`, and writes the corrected text back through the mod's own config files. A `Server`
    /// config may have come from the world's `serverconfig/`, which those files don't reach, so
    /// it is corrected in memory only. True when it was this store's file.
    pub fn accept(&self, mod_id: &str, file: &str, config: &ConfigFile, files: &dyn mod_api::ConfigFiles) -> bool {
        if config.mod_id != mod_id || config.file != file {
            return false;
        }
        let loaded = self.load(&config.text);
        if matches!(config.kind, ConfigKind::Server) {
            return true;
        }
        if let (Some(text), Ok(path)) = (loaded.corrected, mod_api::ConfigPath::new(file)) {
            let _ = files.write(&path, text.as_bytes());
        }
        true
    }

    /// Back to the defaults (`config_unloading` of a server config).
    pub fn reset(&self) {
        *self.values.write().unwrap_or_else(|e| e.into_inner()) = self.spec.defaults();
    }

    /// The value at `path`. Panics for a path the spec doesn't define: a typo in the port.
    pub fn get(&self, path: &str) -> Value {
        self.values
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(path)
            .cloned()
            .unwrap_or_else(|| panic!("config has no value {path}"))
    }

    pub fn bool(&self, path: &str) -> bool {
        match self.get(path) {
            Value::Bool(b) => b,
            v => panic!("config value {path} is not a bool: {v:?}"),
        }
    }

    pub fn int(&self, path: &str) -> i64 {
        match self.get(path) {
            Value::Int(i) => i,
            v => panic!("config value {path} is not an int: {v:?}"),
        }
    }

    pub fn double(&self, path: &str) -> f64 {
        match self.get(path) {
            Value::Double(d) => d,
            Value::Int(i) => i as f64,
            v => panic!("config value {path} is not a double: {v:?}"),
        }
    }

    pub fn string(&self, path: &str) -> String {
        match self.get(path) {
            Value::Str(s) => s,
            v => panic!("config value {path} is not a string: {v:?}"),
        }
    }

    pub fn strings(&self, path: &str) -> Vec<String> {
        match self.get(path) {
            Value::List(items) => items
                .into_iter()
                .filter_map(|i| match i {
                    Value::Str(s) => Some(s),
                    _ => None,
                })
                .collect(),
            v => panic!("config value {path} is not a list: {v:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> Spec {
        Spec::builder()
            .comment("Enable debug output")
            .define_bool("debug", false)
            .comment("General settings")
            .push("general")
            .comment("Radius in blocks")
            .define_int("radius", 64, 1, 512)
            .define_double("chance", 0.5, 0.0, 1.0)
            .define_enum("mode", "CUBE", &["CUBE", "SPHERE"])
            .define_str_list("blocked", &["minecraft:zombie"])
            .pop()
            .build()
    }

    #[test]
    fn defaults_written_then_read_back_unchanged() {
        let spec = spec();
        let text = spec.write(&spec.defaults());
        let loaded = spec.read(&text);
        assert_eq!(loaded.values, spec.defaults());
        assert_eq!(loaded.corrected, None, "{text}");
    }

    #[test]
    fn the_file_shows_comments_ranges_and_allowed_values() {
        let spec = spec();
        let text = spec.write(&spec.defaults());
        assert_eq!(
            text,
            "#Enable debug output\ndebug = false\n\n#General settings\n[general]\n\t#Radius in blocks\n\t#Range: 1 ~ 512\n\tradius = 64\n\t#Range: 0.0 ~ 1.0\n\tchance = 0.5\n\t#Allowed Values: CUBE, SPHERE\n\tmode = \"CUBE\"\n\tblocked = [\"minecraft:zombie\"]\n"
        );
    }

    #[test]
    fn a_players_values_are_kept_and_bad_ones_corrected() {
        let spec = spec();
        let loaded = spec.read(
            "debug = true\n[general]\nradius = 9000\nchance = 1\nmode = \"SPHERE\"\nblocked = []\nold_key = 3\n",
        );
        assert_eq!(loaded.values["debug"], Value::Bool(true));
        assert_eq!(loaded.values["general.radius"], Value::Int(64));
        assert_eq!(loaded.values["general.chance"], Value::Double(1.0));
        assert_eq!(loaded.values["general.mode"], Value::Str("SPHERE".into()));
        assert_eq!(loaded.values["general.blocked"], Value::List(vec![]));
        assert_eq!(loaded.fixed, vec!["general.radius".to_owned()]);
        let corrected = loaded.corrected.expect("radius and the unknown key are corrected");
        assert!(!corrected.contains("old_key"));
        assert!(corrected.contains("radius = 64"));
    }

    #[test]
    fn an_unreadable_file_becomes_the_defaults() {
        let spec = spec();
        let loaded = spec.read("[general\nradius = ");
        assert_eq!(loaded.values, spec.defaults());
        assert!(loaded.corrected.is_some());
    }

    #[test]
    fn a_store_reads_defaults_until_loaded() {
        let store = Store::new(spec());
        assert_eq!(store.int("general.radius"), 64);
        store.load("[general]\nradius = 12\n");
        assert_eq!(store.int("general.radius"), 12);
        assert_eq!(store.strings("general.blocked"), vec!["minecraft:zombie"]);
        store.reset();
        assert_eq!(store.int("general.radius"), 64);
    }
}
