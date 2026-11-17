//! Checks of the port's data files against what Horizon's content loader accepts: the keys of item
//! and block files, the component types (the loader knows only those some vanilla item has by
//! default, so a component such as `minecraft:unbreakable` stops the start), the recipes' ids and the
//! names in the lang file. They read `data/` and `assets/` of this folder.
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

const NS: &str = "mysticalagradditions";
/// Ids of other ported mods that recipes may name without a `neoforge:mod_loaded` guard.
const EXTERNAL: &[&str] = &[
    "inferium",
    "prudentium",
    "tertium",
    "imperium",
    "supremium",
    "earth",
    "air",
    "water",
    "fire",
    "stone",
    "coal",
    "iron",
    "gold",
    "redstone",
    "diamond",
    "emerald",
];

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
fn recipes_name_only_items_that_exist_or_that_a_mod_loaded_condition_guards() {
    let own = own_items();
    let recipes = json_files(&root().join(format!("data/{NS}/recipe")));
    assert!(recipes.len() > 20);
    for (name, r) in recipes {
        let kind = r["type"].as_str().unwrap();
        assert!(
            kind.starts_with("minecraft:"),
            "{name}: type {kind} has no serializer here"
        );
        let guarded = r
            .get("neoforge:conditions")
            .and_then(Value::as_array)
            .is_some_and(|c| c.iter().all(|c| c["type"] == "neoforge:mod_loaded") && !c.is_empty());
        let mut ids = Vec::new();
        for field in [
            "key",
            "ingredients",
            "ingredient",
            "base",
            "addition",
            "template",
            "result",
        ] {
            if let Some(v) = r.get(field) {
                let mut found = Vec::new();
                strings(v, &mut found);
                ids.extend(found.into_iter().filter(|s| s.contains(':')));
            }
        }
        for id in ids {
            if id.starts_with('#') || id.starts_with("minecraft:") || own.contains(&id) || guarded {
                continue;
            }
            let ok = id.strip_prefix("mysticalagriculture:").is_some_and(|rest| {
                EXTERNAL.iter().any(|c| {
                    rest.strip_prefix(c)
                        .is_some_and(|r| ["_essence", "_seeds", "_farmland"].contains(&r))
                })
            });
            assert!(ok, "{name}: {id} exists in no ported mod");
        }
    }
}

#[test]
fn paxels_mine_what_an_axe_a_pickaxe_and_a_shovel_mine() {
    let paxels: Vec<_> = horizon("item")
        .into_iter()
        .filter(|(n, _)| n.ends_with("_paxel"))
        .collect();
    assert_eq!(paxels.len(), 6);
    for (name, v) in paxels {
        let tool = &v["components"]["minecraft:tool"];
        let blocks: BTreeSet<&str> = tool["rules"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|r| r["blocks"].as_str())
            .collect();
        for tag in [
            "#minecraft:mineable/pickaxe",
            "#minecraft:mineable/axe",
            "#minecraft:mineable/shovel",
        ] {
            assert!(blocks.contains(tag), "{name} lacks {tag}");
        }
        assert!(
            v["components"]["minecraft:max_damage"].as_u64().unwrap() >= 345,
            "{name}"
        );
    }
}
