//! Checks of the port's data files against what Horizon's content loader accepts: the keys of item
//! and block files, the component types (the loader knows only those some vanilla item has by
//! default, so a component such as `minecraft:unbreakable` stops the start), the recipes' ids and the
//! names in the lang file. They read `data/` and `assets/` of this folder.
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

const NS: &str = "justdirethings";
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
    "minecraft:cooking_fuel",
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
    let recipes = json_files(&root().join(format!("data/{NS}/recipe")));
    assert!(recipes.len() > 100);
    for (name, r) in recipes {
        let kind = r["type"].as_str().unwrap();
        assert!(
            kind.starts_with("minecraft:") || kind == "justdirethings:goospread",
            "{name}: type {kind} has no serializer here"
        );
        if kind == "justdirethings:goospread" {
            continue;
        }
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
            assert!(
                id.starts_with('#') || id.starts_with("minecraft:") || own.contains(&id),
                "{name}: {id} exists in no ported mod"
            );
        }
        let text = r.to_string();
        assert!(
            !text.contains("\"item\":") && !text.contains("\"tag\":"),
            "{name}: old ingredient form"
        );
    }
}

#[test]
fn fuels_burn_as_long_as_the_originals_data_map_says() {
    let expected = [
        ("item/coal_t1", 4800),
        ("item/coal_t2", 14_400),
        ("item/coal_t3", 43_200),
        ("item/coal_t4", 129_600),
        ("block/coalblock_t1", 48_000),
        ("block/coalblock_t2", 144_000),
        ("block/coalblock_t3", 432_000),
        ("block/coalblock_t4", 1_296_000),
        ("block/charcoal", 16_000),
    ];
    for (file, burn) in expected {
        let path = root().join(format!("data/{NS}/horizon/{file}.json"));
        let v: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(
            v["components"]["minecraft:cooking_fuel"]["burn_time"], burn,
            "{file}"
        );
    }
}

#[test]
fn tools_and_armor_load_with_their_numbers() {
    let items = horizon("item");
    let get = |n: &str| {
        items
            .iter()
            .find(|(name, _)| name == n)
            .map(|(_, v)| v.clone())
            .unwrap()
    };
    let pick = get("blazegold_pickaxe");
    assert_eq!(pick["template"], "minecraft:iron_pickaxe");
    assert_eq!(
        pick["components"]["minecraft:tool"]["rules"][0]["blocks"],
        "#minecraft:incorrect_for_diamond_tool"
    );
    let helmet = get("eclipsealloy_helmet");
    assert_eq!(helmet["components"]["minecraft:max_damage"], 11 * 60);
    assert!(
        helmet["components"]["minecraft:attribute_modifiers"]
            .as_array()
            .unwrap()
            .len()
            == 3
    );
    // a paxel mines what a pickaxe, an axe and a shovel mine
    let paxel = get("celestigem_paxel");
    let rules = paxel["components"]["minecraft:tool"]["rules"]
        .as_array()
        .unwrap();
    assert_eq!(rules.len(), 4);
}
