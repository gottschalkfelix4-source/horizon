#!/usr/bin/env python3
"""Writes data/ and assets/ of the Create Additions port from the original's data files
(`gen_data.py <folder with the unpacked data.tar.gz of the jar's assets/ and data/>`): recipes,
loot tables, tags and translations upgraded to 26.3's formats. Run from this folder."""
import os
import re
import sys

import port_convert as pc

SRC = sys.argv[1]
NS = "createaddition"
pc.OWN.add(NS)

# block: (template, hardness, resistance, item)
BLOCKS = {
    "electric_motor": ("minecraft:observer", 3.0, 6.0),
    "alternator": ("minecraft:observer", 3.0, 6.0),
    "servo_motor": ("minecraft:observer", 3.0, 6.0),
    "rolling_mill": ("minecraft:furnace", 3.0, 6.0),
    "tesla_coil": ("minecraft:furnace", 3.0, 6.0),
    "modular_accumulator": ("minecraft:iron_block", 3.0, 6.0),
    "creative_energy": ("minecraft:bedrock", -1.0, 3600000.0),
    "connector": ("minecraft:iron_bars", 1.0, 3.0),
    "large_connector": ("minecraft:iron_bars", 1.0, 3.0),
    "small_light_connector": ("minecraft:iron_bars", 1.0, 3.0),
    "redstone_relay": ("minecraft:observer", 3.0, 6.0),
    "portable_energy_interface": ("minecraft:observer", 3.0, 6.0),
    "digital_adapter": ("minecraft:observer", 3.0, 6.0),
    "electric_pump": ("minecraft:observer", 3.0, 6.0),
    "barbed_wire": ("minecraft:iron_bars", 2.0, 3.0),
    "electrum_block": ("minecraft:iron_block", 5.0, 6.0),
    "biomass_pellet_block": ("minecraft:hay_block", 0.5, 0.5),
    "chocolate_cake": ("minecraft:cake", None, None),
    "honey_cake": ("minecraft:cake", None, None),
}
ITEMS = {
    "electrum_ingot": "minecraft:gold_ingot", "electrum_nugget": "minecraft:gold_nugget",
    "electrum_sheet": "minecraft:gold_ingot", "zinc_sheet": "minecraft:iron_ingot",
    "iron_rod": "minecraft:stick", "gold_rod": "minecraft:stick", "copper_rod": "minecraft:stick",
    "brass_rod": "minecraft:stick", "bronze_rod": "minecraft:stick", "electrum_rod": "minecraft:stick",
    "iron_wire": "minecraft:string", "gold_wire": "minecraft:string", "copper_wire": "minecraft:string",
    "electrum_wire": "minecraft:string",
    "spool": "minecraft:string", "copper_spool": "minecraft:string", "gold_spool": "minecraft:string",
    "electrum_spool": "minecraft:string", "festive_spool": "minecraft:string",
    "capacitor": "minecraft:redstone", "biomass": "minecraft:wheat", "biomass_pellet": "minecraft:coal",
    "straw": "minecraft:stick", "diamond_grit": "minecraft:diamond",
    "diamond_grit_sandpaper": "minecraft:paper", "cake_base": "minecraft:bread",
    "cake_base_baked": "minecraft:bread", "brass_figurine": "minecraft:gold_nugget",
    "copper_goblet": "minecraft:copper_ingot", "gold_goblet": "minecraft:gold_ingot",
    "electrum_amulet": "minecraft:gold_ingot",
}
for name, (template, hardness, resistance) in BLOCKS.items():
    body = {"template": template, "item": True}
    if hardness is not None:
        body["properties"] = {"hardness": hardness, "explosion_resistance": resistance}
    pc.write(f"data/{NS}/horizon/block/{name}.json", body)
for name, template in ITEMS.items():
    pc.write(f"data/{NS}/horizon/item/{name}.json", {"template": template})

# What Create's port declares: recipes naming other Create items are left out.
CREATE = set()
for kind in ("block", "item"):
    d = f"../create/data/create/horizon/{kind}"
    for n in os.listdir(d):
        CREATE.add("create:" + n[:-5])
OWN_ITEMS = {f"{NS}:{n}" for n in ITEMS} | {f"{NS}:{n}" for n in BLOCKS}
CUSTOM = {"createaddition:charging", "createaddition:rolling"}


def exists(i):
    return i in OWN_ITEMS or i in CREATE or (i.startswith("minecraft:") and i != "minecraft:air")


def fix_result(res):
    """1.21.1 wrote stored enchantments as {"levels": {...}}; 26.3 as the map itself."""
    comp = res.get("components", {})
    se = comp.get("minecraft:stored_enchantments")
    if isinstance(se, dict) and "levels" in se:
        comp["minecraft:stored_enchantments"] = se["levels"]
    return res


kept, skipped = [], []
for rel, recipe in pc.files(SRC, f"data/{NS}/recipe"):
    if rel.split("/")[0] in ("compat", "liquid_burning", "compacting", "crushing", "filling", "mixing",
                             "pressing", "mechanical_crafting"):
        skipped.append((rel, "another mod's recipe type"))
        continue
    ok, left = pc.condition_holds(recipe.get("neoforge:conditions", []), exists)
    if not ok or any(c.get("type", "").startswith("createaddition:") for c in left):
        skipped.append((rel, "condition"))
        continue
    if recipe["type"] in CUSTOM:
        new = dict(recipe)
        new["ingredients"] = []
        for v in recipe["ingredients"]:
            i = pc.ingredient(v)
            if i is None:
                break
            new["ingredients"].append(i)
        else:
            new["results"] = [fix_result(r) for r in recipe["results"]]
            needs = [n for i in new["ingredients"] for n in pc.ids_in(i) if not n.startswith("#")]
            needs += [r["id"] for r in new["results"]]
            if any(not exists(n) for n in needs):
                skipped.append((rel, "needs an item that is not ported"))
                continue
            new.pop("neoforge:conditions", None)
            pc.write(f"data/{NS}/recipe/{rel}.json", new)
            kept.append(rel)
            continue
        skipped.append((rel, "ingredient"))
        continue
    new = pc.upgrade_recipe(recipe)
    if new is None:
        skipped.append((rel, "type " + recipe["type"]))
        continue
    needs, res = pc.recipe_ids(new)
    missing = [n for n in needs + [res] if not n.startswith("#") and not exists(n)]
    if missing:
        skipped.append((rel, "needs " + ", ".join(missing)))
        continue
    if left:
        new["neoforge:conditions"] = left
    else:
        new.pop("neoforge:conditions", None)
    pc.write(f"data/{NS}/recipe/{rel}.json", new)
    kept.append(rel)
print("recipes kept", len(kept), "skipped", len(skipped))
for rel, why in skipped:
    if not rel.startswith("charging/deoxidize"):
        print("  skip", rel, "-", why)

# loot tables
for rel, table in pc.files(SRC, f"data/{NS}/loot_table/blocks"):
    if rel in BLOCKS:
        pc.write(f"data/{NS}/loot_table/blocks/{rel}.json", pc.upgrade_loot(table))
print("loot tables for", sorted(set(BLOCKS) - {n for n, _ in pc.files(SRC, f'data/{NS}/loot_table/blocks')}), "missing")

# tags: this mod's, the shared c: and minecraft: ones, and Create's
for root, _dirs, names in os.walk(f"{SRC}/data"):
    for n in names:
        if n.startswith("._") or not n.endswith(".json"):
            continue
        full = os.path.join(root, n)
        rel = os.path.relpath(full, f"{SRC}/data")
        parts = rel.split("/")
        if parts[0] not in ("c", "minecraft", NS) or parts[1] != "tags":
            continue
        kind = parts[2]
        pool = BLOCKS if kind == "block" else ITEMS
        own_names = {f"{NS}:{k}" for k in (set(BLOCKS) | set(ITEMS) if kind == "item" else BLOCKS)}
        tag = pc.read(full)
        tag["values"] = pc.tag_values(
            tag.get("values", []),
            lambda i: i in own_names or i in CREATE or (i.startswith("minecraft:") and not i.startswith("minecraft:?")))
        # another mod's entries stay optional; Create's missing ones too
        tag["values"] = [
            ({"id": v, "required": False} if isinstance(v, str) and v.startswith("create:") and v not in CREATE else v)
            for v in tag["values"]]
        if tag["values"]:
            pc.write(f"data/{rel}", tag)

# lang
lang = pc.read(f"{SRC}/assets/{NS}/lang/en_us.json")
out = {}
for k, v in lang.items():
    m = re.match(r"(item|block)\.createaddition\.(.+)$", k)
    if m:
        name = m.group(2)
        if (m.group(1) == "item" and (name in ITEMS or name in BLOCKS)) or (m.group(1) == "block" and name in BLOCKS):
            out[k] = v
    else:
        out[k] = v
out[f"message.{NS}.motor_speed"] = "Motor speed: %s RPM"
pc.write(f"assets/{NS}/lang/en_us.json", out)
