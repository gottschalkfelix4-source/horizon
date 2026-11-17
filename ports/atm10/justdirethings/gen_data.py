#!/usr/bin/env python3
"""Writes data/ and assets/ of the Just Dire Things port from the original's data files
(`gen_data.py <folder with the unpacked data.tar.gz of the jar's assets/ and data/>`): the item and block
names of its lang file, recipes, loot tables, tags, fuel burn times and the goo recipes, upgraded to
26.3's formats. Tool and armor numbers are assumptions (the original sets them in code); see README.md.
Run from this folder."""
import os
import re
import sys

import port_convert as pc

SRC = sys.argv[1]
NS = "justdirethings"
pc.OWN.add(NS)

# material: (tool tier, tool durability, mining speed, attack bonus, armor durability factor,
#            defense head/chest/legs/boots, toughness, knockback resistance, bow durability)
MATS = {
    "ferricore": ("iron", 1000, 7.0, 2.0, 22, (2, 6, 5, 2), 0.0, 0.0, 500),
    "blazegold": ("diamond", 2500, 9.0, 3.0, 36, (3, 8, 6, 3), 2.0, 0.0, 900),
    "celestigem": ("netherite", 4000, 11.0, 4.0, 44, (4, 9, 7, 4), 3.0, 0.05, 1500),
    "eclipsealloy": ("netherite", 8000, 14.0, 6.0, 60, (5, 11, 8, 5), 4.0, 0.1, 3000),
}
TOOL_BASE = {"sword": (3.0, -2.4), "pickaxe": (1.0, -2.8), "axe": (5.0, -3.0), "shovel": (1.5, -3.0),
             "hoe": (0.0, -1.0), "paxel": (5.0, -3.0)}
ARMOR = {"helmet": ("head", 11, 0), "chestplate": ("chest", 16, 1), "leggings": ("legs", 15, 2),
         "boots": ("feet", 13, 3)}
FUEL = {  # the original's furnace fuel data map
    "coal_t1": 4800, "coal_t2": 14400, "coal_t3": 43200, "coal_t4": 129600,
    "coalblock_t1": 48000, "coalblock_t2": 144000, "coalblock_t3": 432000, "coalblock_t4": 1296000,
    "charcoal": 16000,
}
FUEL_FILE = pc.read(f"{SRC}/data/neoforge/data_maps/item/furnace_fuels.json")["values"]
for k, v in FUEL_FILE.items():
    assert FUEL[k.split(":")[1]] == v["burn_time"], k


def modifier(kind, amount, slot, ident=None):
    return {"type": f"minecraft:{kind}", "id": ident or f"minecraft:base_{kind}", "amount": amount,
            "operation": "add_value", "slot": slot}


def tool(mat, kind):
    tier, dur, speed, bonus, *_ = MATS[mat]
    damage, attack_speed = TOOL_BASE[kind]
    comp = {
        "minecraft:max_damage": dur,
        "minecraft:attribute_modifiers": [modifier("attack_damage", damage + bonus, "mainhand"),
                                          modifier("attack_speed", attack_speed, "mainhand")],
    }
    if kind != "sword":
        kinds = ["pickaxe", "axe", "shovel"] if kind == "paxel" else [kind]
        rules = [{"blocks": f"#minecraft:incorrect_for_{tier}_tool", "correct_for_drops": False}]
        rules += [{"blocks": f"#minecraft:mineable/{k}", "correct_for_drops": True, "speed": speed} for k in kinds]
        comp["minecraft:tool"] = {"rules": rules, "default_mining_speed": 1.0, "damage_per_block": 1}
    template = "minecraft:iron_pickaxe" if kind == "paxel" else f"minecraft:iron_{kind}"
    return {"template": template, "components": comp}


def armor(mat, piece):
    _, _, _, _, factor, defense, toughness, kb, _ = MATS[mat]
    slot, base, i = ARMOR[piece]
    ident = f"{NS}:{mat}_{piece}"
    mods = [{"type": "minecraft:armor", "id": ident, "amount": defense[i], "operation": "add_value", "slot": slot},
            {"type": "minecraft:armor_toughness", "id": ident, "amount": toughness,
             "operation": "add_value", "slot": slot}]
    if kb:
        mods.append({"type": "minecraft:knockback_resistance", "id": ident, "amount": kb,
                     "operation": "add_value", "slot": slot})
    return {"template": f"minecraft:iron_{piece}",
            "components": {"minecraft:max_damage": base * factor, "minecraft:attribute_modifiers": mods}}


def fuel(name, template):
    return {"template": template, "components": {"minecraft:cooking_fuel": {
        "burn_time": FUEL[name], "speed_multiplier": 1.0}}}


def item_spec(name):
    for mat in MATS:
        if name.startswith(mat + "_"):
            rest = name[len(mat) + 1:]
            if rest in TOOL_BASE:
                return tool(mat, rest)
            if rest in ARMOR:
                return armor(mat, rest)
    if name.startswith("bow_") and name[4:] in MATS:
        return {"template": "minecraft:bow", "components": {"minecraft:max_damage": MATS[name[4:]][8]}}
    if name.startswith("template_"):
        return {"template": "minecraft:netherite_upgrade_smithing_template"}
    if name.startswith("upgrade_"):
        return {"template": "minecraft:paper"}
    if name in ("coal_t1", "coal_t2", "coal_t3", "coal_t4"):
        return fuel(name, "minecraft:coal")
    if name.endswith("_ingot"):
        return {"template": "minecraft:iron_ingot"}
    if name.startswith("raw_"):
        return {"template": "minecraft:raw_iron"}
    if name in ("celestigem", "time_crystal"):
        return {"template": "minecraft:diamond" if name == "celestigem" else "minecraft:amethyst_shard"}
    if name.endswith("canister"):
        return {"template": "minecraft:glass_bottle"}
    return {"template": "minecraft:stick"}


def block_spec(name):
    if name in ("coalblock_t1", "coalblock_t2", "coalblock_t3", "coalblock_t4", "charcoal"):
        body = {"template": "minecraft:coal_block", "properties": {"hardness": 5.0, "explosion_resistance": 6.0},
                "item": True}
        body["components"] = {"minecraft:cooking_fuel": {"burn_time": FUEL[name], "speed_multiplier": 1.0}}
        return body
    if name.startswith("raw_") and name.endswith("_ore"):
        template = "minecraft:coal_ore" if "coal" in name else "minecraft:stone"
        return {"template": template, "properties": {"hardness": 3.0, "explosion_resistance": 3.0}, "item": True,
                "state_properties": {"facing": {"type": "enum", "values": [
                    "north", "east", "south", "west", "up", "down"]}}}
    if name.endswith("_block") and name.split("_")[0] in MATS:
        return {"template": "minecraft:iron_block", "properties": {"hardness": 5.0, "explosion_resistance": 6.0},
                "item": True}
    if name.startswith("gooblock_"):
        return {"template": "minecraft:slime_block", "item": True}
    if name.startswith("goosoil_"):
        return {"template": "minecraft:dirt", "properties": {"hardness": 0.5, "explosion_resistance": 0.5},
                "item": True}
    if name == "time_crystal_block" or name == "time_crystal_budding_block":
        return {"template": "minecraft:amethyst_block", "item": True}
    if name.startswith("time_crystal_cluster"):
        return {"template": "minecraft:amethyst_cluster", "item": True}
    return {"template": "minecraft:observer", "properties": {"hardness": 3.0, "explosion_resistance": 6.0},
            "item": True}


lang = pc.read(f"{SRC}/assets/{NS}/lang/en_us.json")
SKIP = re.compile(r"(_bucket|_fluid_block|_fluid)$")
items, blocks = {}, {}
for k in lang:
    m = re.match(r"(item|block)\.justdirethings\.(.+)$", k)
    if not m:
        continue
    kind, name = m.groups()
    if SKIP.search(name) or name in ("xp_fluid_block", "portal_fluid_block", "polymorphic_fluid_block"):
        continue
    (items if kind == "item" else blocks)[name] = None
# the Totem of Death Recall must not become a totem of undying through its template
for name in items:
    items[name] = item_spec(name)
for name in blocks:
    blocks[name] = block_spec(name)
# machines whose fluid or time blocks are not ported are still declared; fluid blocks are not
for name, body in blocks.items():
    pc.write(f"data/{NS}/horizon/block/{name}.json", body)
for name, body in items.items():
    pc.write(f"data/{NS}/horizon/item/{name}.json", body)

OWN_ITEMS = {f"{NS}:{n}" for n in items} | {f"{NS}:{n}" for n in blocks}
OWN_BLOCKS = {f"{NS}:{n}" for n in blocks}


def exists(i):
    return i in OWN_ITEMS or (i.startswith("minecraft:") and i != "minecraft:air")


# recipes
kept, skipped = 0, {}
for rel, recipe in pc.files(SRC, f"data/{NS}/recipe"):
    kind = recipe["type"]
    if kind == "justdirethings:goospread":
        inp, out = recipe["input"]["Name"], recipe["output"]["Name"]
        if (not exists(inp) and inp not in OWN_BLOCKS) or out not in OWN_BLOCKS:
            skipped[rel] = "block not ported"
            continue
        new = {"type": kind, "id": recipe["id"], "input": recipe["input"], "output": recipe["output"],
               "crafting_duration": recipe["craftingDuration"], "tier_requirement": recipe["tierRequirement"]}
        pc.write(f"data/{NS}/recipe/{rel}.json", new)
        kept += 1
        continue
    new = pc.upgrade_recipe(recipe)
    if new is None:
        skipped[rel] = "type " + kind
        continue
    needs, res = pc.recipe_ids(new)
    if any(not n.startswith("#") and not exists(n) for n in needs + [res]):
        skipped[rel] = "item not ported"
        continue
    conds, left = recipe.get("neoforge:conditions", []), []
    if conds:
        ok, left = pc.condition_holds(conds, exists)
        if not ok:
            continue
    new.pop("neoforge:conditions", None)
    if left:
        new["neoforge:conditions"] = left
    pc.write(f"data/{NS}/recipe/{rel}.json", new)
    kept += 1
by = {}
for why in skipped.values():
    by[why] = by.get(why, 0) + 1
print("recipes kept", kept, "skipped", by)

# loot tables
for rel, table in pc.files(SRC, f"data/{NS}/loot_table/blocks"):
    if f"{NS}:{rel}" in OWN_BLOCKS:
        table.pop("random_sequence", None)
        pc.write(f"data/{NS}/loot_table/blocks/{rel}.json", pc.upgrade_loot(table))
print("blocks without a loot table:", sorted(
    n for n in blocks if not os.path.exists(f"data/{NS}/loot_table/blocks/{n}.json")))

# tags (this mod's block and item tags and the shared ones; entity and biome tags are not carried)
for root, _dirs, names in os.walk(f"{SRC}/data"):
    for n in names:
        if n.startswith("._") or not n.endswith(".json"):
            continue
        full = os.path.join(root, n)
        rel = os.path.relpath(full, f"{SRC}/data")
        parts = rel.split("/")
        if parts[0] not in ("c", "minecraft", NS) or parts[1] != "tags" or parts[2] not in ("block", "item"):
            continue
        pool = OWN_BLOCKS if parts[2] == "block" else OWN_ITEMS
        tag = pc.read(full)
        tag["values"] = pc.tag_values(tag.get("values", []), lambda i: i in pool or i.startswith("minecraft:"))
        if tag["values"]:
            pc.write(f"data/{rel}", tag)
pc.write("data/minecraft/tags/block/mineable/pickaxe.json",
         {"replace": False, "values": sorted(f"{NS}:{n}" for n in blocks if not n.startswith("goo"))})
pc.write("data/minecraft/tags/block/mineable/shovel.json",
         {"replace": False, "values": sorted(f"{NS}:{n}" for n in blocks if n.startswith("goo"))})

# lang
out = {}
for k, v in lang.items():
    m = re.match(r"(item|block)\.justdirethings\.(.+)$", k)
    if m:
        kind, name = m.groups()
        if name in (items if kind == "item" else blocks) or (kind == "item" and name in blocks):
            out[k] = v
    else:
        out[k] = v
pc.write(f"assets/{NS}/lang/en_us.json", out)
