#!/usr/bin/env python3
"""Writes data/ae2/horizon/{block,item}/*.json: the plain content of the stage-1 port (templates are
placeholders until `horizon-port-pack carry`). Run from this folder."""
import json, os

base = "data/ae2/horizon"
os.makedirs(f"{base}/block", exist_ok=True)
os.makedirs(f"{base}/item", exist_ok=True)

def block(name, template, hardness, resistance, light=None):
    props = {"hardness": hardness, "explosion_resistance": resistance}
    if light:
        props["light"] = light
    with open(f"{base}/block/{name}.json", "w") as f:
        json.dump({"template": template, "properties": props, "item": True}, f, indent=2)
        f.write("\n")

def item(name, template="minecraft:iron_ingot"):
    with open(f"{base}/item/{name}.json", "w") as f:
        json.dump({"template": template}, f)
        f.write("\n")

colors = ["fluix", "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray",
          "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black"]
for c in colors:
    for cable in ["glass_cable", "covered_cable", "smart_cable", "covered_dense_cable", "smart_dense_cable"]:
        block(f"{c}_{cable}", "minecraft:iron_bars", 0.5, 0.5)
block("controller", "minecraft:iron_block", 4.0, 6.0)
block("energy_acceptor", "minecraft:iron_block", 2.5, 6.0)
block("energy_cell", "minecraft:iron_block", 2.5, 6.0)
block("dense_energy_cell", "minecraft:iron_block", 2.5, 6.0)
block("quartz_block", "minecraft:quartz_block", 0.8, 0.8)
block("fluix_block", "minecraft:quartz_block", 0.8, 0.8)
block("quartz_ore", "minecraft:iron_ore", 3.0, 3.0)
block("deepslate_quartz_ore", "minecraft:deepslate_iron_ore", 4.5, 3.0)
for crystal in ["certus_quartz_crystal", "charged_certus_quartz_crystal", "fluix_crystal"]:
    item(crystal, "minecraft:quartz")
for dust in ["certus_quartz_dust", "fluix_dust", "ender_dust", "sky_dust"]:
    item(dust, "minecraft:gunpowder")
item("silicon")
for p in ["calculation", "engineering", "logic", "silicon"]:
    item(f"printed_{p}_processor" if p != "silicon" else "printed_silicon")
for p in ["calculation", "engineering", "logic"]:
    item(f"{p}_processor")
item("fluix_pearl", "minecraft:ender_pearl")

block("drive", "minecraft:furnace", 2.2, 6.0)
for t in ["1k", "4k", "16k", "64k", "256k"]:
    with open(f"{base}/item/item_storage_cell_{t}.json", "w") as f:
        json.dump({"template": "minecraft:iron_ingot", "max_stack_size": 1}, f)
        f.write("\n")
item("cell_component_1k"); item("cell_component_4k"); item("cell_component_16k"); item("cell_component_64k"); item("cell_component_256k")
item("empty_storage_cell")

# block tags: every block is mined with a pickaxe, the ores need a stone one
tag_dir = "data/minecraft/tags/block"
os.makedirs(f"{tag_dir}/mineable", exist_ok=True)
names = sorted(f[:-5] for f in os.listdir(f"{base}/block"))
with open(f"{tag_dir}/mineable/pickaxe.json", "w") as f:
    json.dump({"values": [f"ae2:{n}" for n in names]}, f, indent=2)
    f.write("\n")
with open(f"{tag_dir}/needs_stone_tool.json", "w") as f:
    json.dump({"values": [f"ae2:{n}" for n in names if n.endswith("quartz_ore")]}, f, indent=2)
    f.write("\n")
