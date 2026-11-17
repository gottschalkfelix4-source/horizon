#!/usr/bin/env python3
"""Writes data/ and assets/ of the Mystical Agradditions port from the original's data files
(`gen_data.py <folder with the unpacked data.tar.gz of the jar's assets/ and data/>`). Only data
files are read: recipes, loot tables, tags, worldgen, lang and the Tinkers' material stats (used as
the paxels' numbers). Run from this folder."""
import json
import os
import sys

import port_convert as pc

SRC = sys.argv[1]
NS = "mysticalagradditions"
pc.OWN.add(NS)

# Mystical Agriculture's own ids that the MA port (ports/atm10/mysticalagriculture) declares.
MA_CROPS = ["inferium", "prudentium", "tertium", "imperium", "supremium", "earth", "air", "water",
            "fire", "stone", "coal", "iron", "gold", "redstone", "diamond", "emerald"]
MA_IDS = {f"mysticalagriculture:{c}_{k}" for c in MA_CROPS for k in ("essence", "seeds", "crop")}
MA_IDS |= {f"mysticalagriculture:{t}_farmland" for t in
           ("inferium", "prudentium", "tertium", "imperium", "supremium")}

# The six crops this mod adds; in the original they are registered under mysticalagriculture:, here
# under this mod's namespace (a content mod owns only its own).
AWAKENED = {
    "nether_star": "Nether Star",
    "dragon_egg": "Dragon Egg",
    "awakened_draconium": "Awakened Draconium",
    "gaia_spirit": "Gaia Spirit",
    "neutronium": "Neutronium",
    "nitro_crystal": "Nitro Crystal",
}

# block: (template, hardness, resistance, has the item)
BLOCKS = {
    "insanium_block": ("minecraft:iron_block", 5.0, 6.0),
    "insanium_ingot_block": ("minecraft:iron_block", 5.0, 6.0),
    "insanium_gemstone_block": ("minecraft:diamond_block", 5.0, 6.0),
    "insanium_farmland": ("minecraft:farmland", None, None),
    "end_inferium_ore": ("minecraft:end_stone", 3.0, 9.0),
    "nether_inferium_ore": ("minecraft:netherrack", 3.0, 3.0),
}
for tier in ("inferium", "prudentium", "tertium", "imperium", "supremium", "insanium"):
    BLOCKS[f"{tier}_coal_block"] = ("minecraft:coal_block", 5.0, 6.0)
for crux in ("nether_star", "dragon_egg", "gaia_spirit", "awakened_draconium", "neutronium", "nitro_crystal"):
    BLOCKS[f"{crux}_crux"] = ("minecraft:iron_block", 5.0, 6.0)

# Tinkers' head stats of the mod's own data: (durability, mining speed, tier, attack).
PAXELS = {}
for t in ("inferium", "prudentium", "tertium", "imperium", "supremium"):
    head = pc.read(f"{SRC}/data/{NS}/tinkering/materials/stats/{t}.json")["stats"]["tconstruct:head"]
    PAXELS[t] = (head["durability"], head["miningSpeed"], head["harvestTier"].split(":")[1], head["attack"])
# Assumed: the awakened paxel is a quarter above the supremium one.
s = PAXELS["supremium"]
PAXELS["awakened_supremium"] = (round(s[0] * 1.25), round(s[1] * 1.25, 2), "netherite", round(s[3] * 1.25, 2))


def paxel(t):
    dur, speed, tier, attack = PAXELS[t]
    rules = [{"blocks": f"#minecraft:incorrect_for_{tier}_tool", "correct_for_drops": False}]
    for kind in ("pickaxe", "axe", "shovel"):
        rules.append({"blocks": f"#minecraft:mineable/{kind}", "correct_for_drops": True, "speed": speed})
    return {
        "template": "minecraft:iron_pickaxe",
        "components": {
            "minecraft:max_damage": dur,
            "minecraft:tool": {"rules": rules, "default_mining_speed": 1.0, "damage_per_block": 1},
            "minecraft:attribute_modifiers": [
                {"type": "minecraft:attack_damage", "id": "minecraft:base_attack_damage",
                 "amount": attack, "operation": "add_value", "slot": "mainhand"},
                {"type": "minecraft:attack_speed", "id": "minecraft:base_attack_speed",
                 "amount": -3.0, "operation": "add_value", "slot": "mainhand"},
            ],
        },
    }


ITEMS = {}
for t in ("inferium", "prudentium", "tertium", "imperium", "supremium", "insanium"):
    ITEMS[f"{t}_coal"] = {"template": "minecraft:coal"}
    ITEMS[f"{t}_apple"] = {"template": "minecraft:golden_apple" if t in ("inferium", "prudentium")
                           else "minecraft:enchanted_golden_apple"}
for t in PAXELS:
    ITEMS[f"{t}_paxel"] = paxel(t)
ITEMS.update({
    "insanium_essence": {"template": "minecraft:diamond"},
    "insanium_ingot": {"template": "minecraft:iron_ingot"},
    "insanium_nugget": {"template": "minecraft:iron_nugget"},
    "insanium_gemstone": {"template": "minecraft:diamond"},
    "dragon_scale": {"template": "minecraft:phantom_membrane"},
    "dragon_egg_chunk": {"template": "minecraft:amethyst_shard"},
    "nether_star_shard": {"template": "minecraft:quartz"},
    "withering_soul": {"template": "minecraft:ghast_tear"},
    "creative_essence": {"template": "minecraft:nether_star", "rarity": "epic"},
})
for crop in AWAKENED:
    ITEMS[f"{crop}_essence"] = {"template": "minecraft:iron_ingot"}
    ITEMS[f"{crop}_seeds"] = {"template": "minecraft:wheat_seeds", "block": f"{NS}:{crop}_crop"}

# ---------------------------------------------------------------------------------- blocks, items
for name, (template, hardness, resistance) in BLOCKS.items():
    body = {"template": template}
    if hardness is not None:
        body["properties"] = {"hardness": hardness, "explosion_resistance": resistance}
    if name == "insanium_farmland":
        body["base_block"] = "minecraft:dirt"
    body.setdefault("item", True)
    pc.write(f"data/{NS}/horizon/block/{name}.json", body)
for crop in AWAKENED:
    pc.write(f"data/{NS}/horizon/block/{crop}_crop.json",
             {"template": "minecraft:wheat", "item": False, "pick_item": f"{NS}:{crop}_seeds"})
for name, body in ITEMS.items():
    pc.write(f"data/{NS}/horizon/item/{name}.json", body)

BLOCK_IDS = {f"{NS}:{b}" for b in BLOCKS} | {f"{NS}:{c}_crop" for c in AWAKENED}
ITEM_IDS = {f"{NS}:{i}" for i in ITEMS} | {f"{NS}:{b}" for b in BLOCKS}
# the original's ids of the awakened crops' items, moved to this namespace
MOVED = {f"mysticalagriculture:{c}_{k}": f"{NS}:{c}_{k}" for c in AWAKENED for k in ("essence", "seeds")}
HAVE = ITEM_IDS | MA_IDS


def exists(i):
    return i in HAVE or (i.startswith("minecraft:") and i != "minecraft:air")


def moved(v):
    if isinstance(v, str):
        return MOVED.get(v, v)
    if isinstance(v, list):
        return [moved(x) for x in v]
    if isinstance(v, dict):
        return {k: moved(x) for k, x in v.items()}
    return v


# ---------------------------------------------------------------------------------- crop loot
for crop in AWAKENED:
    pc.write(f"data/{NS}/loot_table/blocks/{crop}_crop.json", {
        "pools": [
            {"entries": [{"name": f"{NS}:{crop}_seeds", "type": "minecraft:item"}], "rolls": 1},
            {"condition": {"block": f"{NS}:{crop}_crop", "properties": {"age": "7"},
                           "type": "minecraft:block_state_property"},
             "entries": [{"modifier": [{"count": {"max": 2, "min": 1, "type": "minecraft:uniform"},
                                        "type": "minecraft:set_count"}],
                          "name": f"{NS}:{crop}_essence", "type": "minecraft:item"}],
             "rolls": 1},
        ],
        "type": "minecraft:block",
    })

# ---------------------------------------------------------------------------------- block loot
# insanium farmland breaks into dirt, as farmland does (the original ships no table for it)
pc.write(f"data/{NS}/loot_table/blocks/insanium_farmland.json", {
    "type": "minecraft:block",
    "pools": [{"rolls": 1, "condition": {"type": "minecraft:survives_explosion"},
               "entries": [{"type": "minecraft:item", "name": "minecraft:dirt"}]}]})
dropped = []
for rel, table in pc.files(SRC, f"data/{NS}/loot_table/blocks"):
    if f"{NS}:{rel}" not in BLOCK_IDS:
        continue
    text = json.dumps(table)
    # a table that names an item this port lacks (the prosperity shard) is not carried
    names = {p for p in __import__("re").findall(r'"name": "([^"]+)"', text)}
    if not all(exists(n) for n in names):
        dropped.append(rel)
        continue
    pc.write(f"data/{NS}/loot_table/blocks/{rel}.json", pc.upgrade_loot(moved(table)))

# ---------------------------------------------------------------------------------- recipes
skipped = []
kept = 0
for rel, recipe in pc.files(SRC, f"data/{NS}/recipe"):
    conds = recipe.get("neoforge:conditions", [])
    ok, left = pc.condition_holds(conds, exists, lambda c: True)
    new = pc.upgrade_recipe(moved(recipe))
    if not ok or new is None:
        skipped.append((rel, "type or condition"))
        continue
    needs, res = pc.recipe_ids(new)
    missing = [n for n in needs if not n.startswith("#") and not exists(n)]
    mod_conds = [c.get("modid") for c in left if c.get("type") == "neoforge:mod_loaded"]
    if missing or not exists(res):
        # an ingredient or the result from a mod outside this pack's ported set: kept only when a
        # mod_loaded condition guards it, so the engine drops it where that mod is absent
        if not mod_conds:
            skipped.append((rel, "needs " + ", ".join(missing or [res])))
            continue
    if left:
        new["neoforge:conditions"] = left
    else:
        new.pop("neoforge:conditions", None)
    pc.write(f"data/{NS}/recipe/{rel}.json", new)
    kept += 1
# the farmland and awakening/infusion recipes of Mystical Agriculture's own types are not carried
print("recipes kept", kept, "skipped", len(skipped))
for rel, why in skipped:
    print("  skip", rel, "-", why)
print("loot tables not carried:", dropped)

# ---------------------------------------------------------------------------------- tags
for root, _dirs, names in os.walk(f"{SRC}/data"):
    for n in names:
        if n.startswith("._") or not n.endswith(".json"):
            continue
        full = os.path.join(root, n)
        rel = os.path.relpath(full, f"{SRC}/data")
        parts = rel.split("/")
        if parts[0] not in ("c", "minecraft", NS) or parts[1] != "tags":
            continue
        if parts[0] == "minecraft" and parts[2] == "item" and "cluster" in rel:
            continue
        tag = pc.read(full)
        kind = parts[2]
        known = (lambda i: i in BLOCK_IDS or i.startswith("minecraft:")) if kind == "block" else \
            (lambda i: i in ITEM_IDS or i.startswith("minecraft:"))
        tag["values"] = pc.tag_values(tag.get("values", []), known)
        if not tag["values"]:
            continue
        pc.write(f"data/{rel}", tag)
# the paxels are in the original's tag of the cucumber library, which no port has
if os.path.exists(f"data/cucumber"):
    import shutil
    shutil.rmtree("data/cucumber")

# ---------------------------------------------------------------------------------- worldgen
pc.write(f"data/{NS}/worldgen/feature/end_inferium_ore.json", {
    "type": "minecraft:ore",
    "discard_chance_on_air_exposure": 0.0, "size": 8,
    "targets": [{"state": f"{NS}:end_inferium_ore",
                 "target": {"predicate_type": "minecraft:block_match", "block": "minecraft:end_stone"}}]})
pc.write(f"data/{NS}/worldgen/feature/nether_inferium_ore.json", {
    "type": "minecraft:ore",
    "discard_chance_on_air_exposure": 0.0,
    "size": pc.read(f"{SRC}/data/{NS}/worldgen/configured_feature/nether_inferium_ore.json")["config"]["size"],
    "targets": [{"state": f"{NS}:nether_inferium_ore",
                 "target": {"predicate_type": "minecraft:block_match", "block": "minecraft:netherrack"}}]})
for name, biomes in (("end_inferium_ore", "#minecraft:is_end"), ("nether_inferium_ore", "#minecraft:is_nether")):
    placed = pc.read(f"{SRC}/data/{NS}/worldgen/placed_feature/{name}.json")
    pc.write(f"data/{NS}/worldgen/placed_feature/{name}.json", placed)
    pc.write(f"data/{NS}/neoforge/biome_modifier/{name}.json", {
        "type": "neoforge:add_features", "biomes": biomes, "features": f"{NS}:{name}",
        "step": "underground_ores"})

# ---------------------------------------------------------------------------------- lang
lang = pc.read(f"{SRC}/assets/{NS}/lang/en_us.json")
out = {}
for k, v in lang.items():
    kind, _, rest = k.partition(".")
    if kind in ("item", "block"):
        ns, _, name = rest.partition(".")
        if ns == NS and (f"{NS}:{name}" in (ITEM_IDS if kind == "item" else BLOCK_IDS | ITEM_IDS)):
            out[k] = v
    else:
        out[k] = v
for crop, label in AWAKENED.items():
    out[f"item.{NS}.{crop}_essence"] = f"{label} Essence"
    out[f"item.{NS}.{crop}_seeds"] = f"{label} Seeds"
    out[f"block.{NS}.{crop}_crop"] = f"{label} Crop"
pc.write(f"assets/{NS}/lang/en_us.json", out)
