//! Checks of the port's data files against what Horizon's content loader accepts: the keys of item
//! and block files, the component types (the loader knows only those some vanilla item has by
//! default, so a component such as `minecraft:unbreakable` stops the start), the recipes' ids and the
//! names in the lang file. They read `data/` and `assets/` of this folder.
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

const NS: &str = "createaddition";
/// Items of Create's port (ports/atm10/create), which recipes may name; read from its data folder.
fn create_ids() -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    for kind in ["block", "item"] {
        let dir = root().join(format!("../create/data/create/horizon/{kind}"));
        for e in fs::read_dir(dir).into_iter().flatten().flatten() {
            ids.insert(format!(
                "create:{}",
                e.path().file_stem().unwrap().to_string_lossy()
            ));
        }
    }
    ids
}

/// Component types the content loader knows (a vanilla item holds each by default) and that an item
/// file may patch; `max_stack_size` and `rarity` are keys of the file itself.
const COMPONENTS: &[&str] = &[
    "minecraft:max_damage",
    "minecraft:damage",
    "minecraft:attribute_modifiers",
    "minecraft:tool",
    "minecraft:food",
    "minecraft:consumable",
    "minecraft:repairable",
    "minecraft:enchantable",
    "minecraft:equippable",
    "minecraft:damage_resistant",
    "minecraft:use_remainder",
    "minecraft:use_cooldown",
    "minecraft:item_name",
    "minecraft:lore",
    "minecraft:weapon",
];
const ITEM_KEYS: &[&str] = &[
    "template",
    "max_stack_size",
    "rarity",
    "components",
    "crafting_remainder",
    "block",
    "arm_pose",
];
const BLOCK_KEYS: &[&str] = &[
    "template",
    "properties",
    "shapes",
    "collision_shapes",
    "state_properties",
    "default_state",
    "item",
    "pick_item",
    "block_item",
    "components",
    "base_block",
    "state_light",
    "connects_to",
    "thickness",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn json_files(dir: &Path) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            for (n, v) in json_files(&p) {
                out.push((format!("{}/{n}", e.file_name().to_string_lossy()), v));
            }
        } else if p.extension().is_some_and(|x| x == "json") {
            let text = fs::read_to_string(&p).unwrap();
            let v = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            out.push((p.file_stem().unwrap().to_string_lossy().into_owned(), v));
        }
    }
    out
}

fn horizon(kind: &str) -> Vec<(String, Value)> {
    json_files(&root().join(format!("data/{NS}/horizon/{kind}")))
}

#[test]
fn item_and_block_files_use_only_keys_and_components_the_loader_takes() {
    let items = horizon("item");
    let blocks = horizon("block");
    assert!(!items.is_empty() && !blocks.is_empty());
    for (name, v) in &items {
        let o = v.as_object().unwrap();
        for k in o.keys() {
            assert!(ITEM_KEYS.contains(&k.as_str()), "item {name}: key {k}");
        }
        assert!(
            o["template"].as_str().unwrap().starts_with("minecraft:"),
            "item {name}"
        );
        for k in o
            .get("components")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .map(|(k, _)| k)
        {
            let kind = k.trim_start_matches('!');
            assert!(
                COMPONENTS.contains(&kind),
                "item {name}: component {k} is not loadable"
            );
        }
    }
    for (name, v) in &blocks {
        for k in v.as_object().unwrap().keys() {
            assert!(BLOCK_KEYS.contains(&k.as_str()), "block {name}: key {k}");
        }
    }
}

#[test]
fn every_item_and_block_has_a_name_and_a_block_has_its_loot() {
    let lang: Value = serde_json::from_str(
        &fs::read_to_string(root().join(format!("assets/{NS}/lang/en_us.json"))).unwrap(),
    )
    .unwrap();
    for (name, _) in horizon("item") {
        assert!(
            lang.get(format!("item.{NS}.{name}")).is_some(),
            "no name for item {name}"
        );
    }
    let loot: BTreeSet<String> = json_files(&root().join(format!("data/{NS}/loot_table/blocks")))
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    for (name, v) in horizon("block") {
        assert!(
            lang.get(format!("block.{NS}.{name}")).is_some(),
            "no name for block {name}"
        );
        let _ = v;
        assert!(loot.contains(&name), "no loot table for block {name}");
    }
}

fn own_items() -> BTreeSet<String> {
    let mut ids: BTreeSet<String> = horizon("item")
        .into_iter()
        .map(|(n, _)| format!("{NS}:{n}"))
        .collect();
    for (n, v) in horizon("block") {
        if v.get("item").and_then(Value::as_bool).unwrap_or(true) {
            ids.insert(format!("{NS}:{n}"));
        }
    }
    ids
}

fn strings(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.clone()),
        Value::Array(a) => a.iter().for_each(|x| strings(x, out)),
        Value::Object(o) => o
            .iter()
            .filter(|(k, _)| *k != "type")
            .for_each(|(_, x)| strings(x, out)),
        _ => {}
    }
}

#[test]
fn recipes_use_serializers_the_game_has_and_name_only_items_that_exist() {
    let own = own_items();
    let create = create_ids();
    assert!(!create.is_empty());
    let recipes = json_files(&root().join(format!("data/{NS}/recipe")));
    assert!(recipes.len() > 40);
    for (name, r) in recipes {
        let kind = r["type"].as_str().unwrap();
        assert!(
            kind.starts_with("minecraft:")
                || kind == "createaddition:charging"
                || kind == "createaddition:rolling",
            "{name}: type {kind} has no serializer here"
        );
        let mut ids = Vec::new();
        for field in [
            "key",
            "ingredients",
            "ingredient",
            "base",
            "addition",
            "template",
            "result",
            "results",
        ] {
            if let Some(v) = r.get(field) {
                let mut found = Vec::new();
                strings(v, &mut found);
                ids.extend(found.into_iter().filter(|s| s.contains(':')));
            }
        }
        for id in ids {
            let ok = id.starts_with('#')
                || id.starts_with("minecraft:")
                || own.contains(&id)
                || create.contains(&id);
            assert!(ok, "{name}: {id} exists in no ported mod");
        }
        // a 26.3 ingredient is a string, never {"item": ...}
        let text = r.to_string();
        assert!(
            !text.contains("\"item\":") && !text.contains("\"tag\":"),
            "{name}: old ingredient form"
        );
    }
}
