#!/usr/bin/env python3
"""Writes data/create/horizon/block/*.json, assets/create/lang/en_us.json and the pickaxe/axe tags
for the Create port. Templates are placeholders for look and numbers until `horizon-port-pack
carry`. Run from this folder."""
import json, os

BLOCKS = {
    # name: (template, hardness, resistance, display name)
    "shaft": ("minecraft:stripped_oak_log", 3.0, 6.0, "Shaft"),
    "cogwheel": ("minecraft:stripped_oak_log", 3.0, 6.0, "Cogwheel"),
    "large_cogwheel": ("minecraft:stripped_oak_log", 3.0, 6.0, "Large Cogwheel"),
    "gearbox": ("minecraft:stone", 3.0, 6.0, "Gearbox"),
    "water_wheel": ("minecraft:oak_log", 2.0, 3.0, "Water Wheel"),
    "windmill_bearing": ("minecraft:observer", 3.0, 6.0, "Windmill Bearing"),
    "creative_motor": ("minecraft:observer", 3.0, 6.0, "Creative Motor"),
    "millstone": ("minecraft:furnace", 3.0, 6.0, "Millstone"),
    "belt": ("minecraft:stripped_oak_log", 0.8, 3.0, "Mechanical Belt"),
    "funnel": ("minecraft:observer", 1.0, 3.0, "Andesite Funnel"),
    "crushing_wheel": ("minecraft:furnace", 3.0, 6.0, "Crushing Wheel"),
    "blaze_burner": ("minecraft:furnace", 3.5, 6.0, "Blaze Burner"),
    "andesite_casing": ("minecraft:stripped_oak_log", 2.0, 3.0, "Andesite Casing"),
    "deployer": ("minecraft:stripped_oak_log", 3.5, 6.0, "Deployer"),
    "mechanical_saw": ("minecraft:stripped_oak_log", 3.0, 6.0, "Mechanical Saw"),
    "depot": ("minecraft:stone", 3.0, 6.0, "Depot"),
    "basin": ("minecraft:furnace", 3.0, 6.0, "Basin"),
    "mechanical_mixer": ("minecraft:stripped_oak_log", 3.0, 6.0, "Mechanical Mixer"),
    "mechanical_press": ("minecraft:stripped_oak_log", 3.0, 6.0, "Mechanical Press"),
}
ITEMS = {
    "wheat_flour": ("minecraft:sugar", "Wheat Flour"),
    "iron_sheet": ("minecraft:iron_ingot", "Iron Sheet"),
    "golden_sheet": ("minecraft:gold_ingot", "Golden Sheet"),
    "copper_sheet": ("minecraft:copper_ingot", "Copper Sheet"),
    "brass_sheet": ("minecraft:gold_ingot", "Brass Sheet"),
    "andesite_alloy": ("minecraft:iron_ingot", "Andesite Alloy"),
    "crushed_raw_iron": ("minecraft:raw_iron", "Crushed Raw Iron"),
    "crushed_raw_copper": ("minecraft:raw_copper", "Crushed Raw Copper"),
    "crushed_raw_gold": ("minecraft:raw_gold", "Crushed Raw Gold"),
    "zinc_nugget": ("minecraft:iron_nugget", "Zinc Nugget"),
    "pulp": ("minecraft:sugar", "Pulp"),
    "cardboard": ("minecraft:paper", "Cardboard"),
    "experience_nugget": ("minecraft:gold_nugget", "Nugget of Experience"),
    "powdered_obsidian": ("minecraft:gunpowder", "Powdered Obsidian"),
    "cinder_flour": ("minecraft:gunpowder", "Cinder Flour"),
    "zinc_ingot": ("minecraft:iron_ingot", "Zinc Ingot"),
    "blaze_cake": ("minecraft:cake", "Blaze Cake"),
    "brass_nugget": ("minecraft:gold_nugget", "Brass Nugget"),
    "brass_ingot": ("minecraft:gold_ingot", "Brass Ingot"),
}
base = "data/create/horizon/block"
os.makedirs(base, exist_ok=True)
for name, (template, hardness, resistance, _) in BLOCKS.items():
    with open(f"{base}/{name}.json", "w") as f:
        json.dump({"template": template, "properties": {"hardness": hardness, "explosion_resistance": resistance}, "item": True}, f, indent=2)
        f.write("\n")
os.makedirs("data/create/horizon/item", exist_ok=True)
for name, (template, _) in ITEMS.items():
    with open(f"data/create/horizon/item/{name}.json", "w") as f:
        json.dump({"template": template}, f)
        f.write("\n")
os.makedirs("assets/create/lang", exist_ok=True)
with open("assets/create/lang/en_us.json", "w") as f:
    names = {f"block.create.{n}": v[3] for n, v in BLOCKS.items()}
    names.update({f"item.create.{n}": v[1] for n, v in ITEMS.items()})
    json.dump(names, f, indent=2)
    f.write("\n")
tag = "data/minecraft/tags/block/mineable/axe.json"
os.makedirs(os.path.dirname(tag), exist_ok=True)
with open(tag, "w") as f:
    json.dump({"replace": False, "values": [f"create:{n}" for n in BLOCKS]}, f, indent=2)
    f.write("\n")
