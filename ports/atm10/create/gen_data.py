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
}
base = "data/create/horizon/block"
os.makedirs(base, exist_ok=True)
for name, (template, hardness, resistance, _) in BLOCKS.items():
    with open(f"{base}/{name}.json", "w") as f:
        json.dump({"template": template, "properties": {"hardness": hardness, "explosion_resistance": resistance}, "item": True}, f, indent=2)
        f.write("\n")
os.makedirs("assets/create/lang", exist_ok=True)
with open("assets/create/lang/en_us.json", "w") as f:
    json.dump({f"block.create.{n}": v[3] for n, v in BLOCKS.items()}, f, indent=2)
    f.write("\n")
tag = "data/minecraft/tags/block/mineable/axe.json"
os.makedirs(os.path.dirname(tag), exist_ok=True)
with open(tag, "w") as f:
    json.dump({"replace": False, "values": [f"create:{n}" for n in BLOCKS]}, f, indent=2)
    f.write("\n")
