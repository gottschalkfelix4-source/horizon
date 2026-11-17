#!/usr/bin/env python3
"""Writes data/mekanismgenerators/horizon/block/*.json (placeholders until `carry`)."""
import json, os
d = "data/mekanismgenerators/horizon/block"
os.makedirs(d, exist_ok=True)
for n in ["heat_generator", "solar_generator", "advanced_solar_generator", "wind_generator", "bio_generator"]:
    with open(f"{d}/{n}.json", "w") as f:
        json.dump({"template": "minecraft:furnace" if n in ("heat_generator", "bio_generator") else "minecraft:iron_block", "properties": {"hardness": 3.5, "explosion_resistance": 16.0}, "item": True}, f, indent=2)
        f.write("\n")
