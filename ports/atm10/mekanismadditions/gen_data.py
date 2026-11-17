#!/usr/bin/env python3
"""Writes data/mekanismadditions/horizon/{block,item}/*.json (placeholders until `carry`)."""
import json, os

base = "data/mekanismadditions/horizon"
os.makedirs(f"{base}/block", exist_ok=True)
os.makedirs(f"{base}/item", exist_ok=True)
COLORS = ["black", "red", "green", "brown", "blue", "purple", "cyan", "light_gray", "gray", "pink",
          "lime", "yellow", "light_blue", "magenta", "orange", "white"]


def block(name, template, hardness, resistance, light=None, friction=None):
    props = {"hardness": hardness, "explosion_resistance": resistance}
    if friction:
        props["friction"] = friction
    if light:
        props["light"] = light
    with open(f"{base}/block/{name}.json", "w") as f:
        json.dump({"template": template, "properties": props, "item": True}, f, indent=2)
        f.write("\n")


for c in COLORS:
    block(f"{c}_plastic_block", "minecraft:stone", 5.0, 6.0)
    block(f"{c}_slick_plastic_block", "minecraft:stone", 5.0, 6.0, None, 0.98)
    block(f"{c}_plastic_glow_block", "minecraft:stone", 5.0, 6.0, 15)
    block(f"{c}_reinforced_plastic_block", "minecraft:stone", 50.0, 1200.0)
    block(f"{c}_plastic_road", "minecraft:stone", 5.0, 6.0)
    block(f"{c}_transparent_plastic_block", "minecraft:glass", 5.0, 6.0)
    block(f"{c}_glow_panel", "minecraft:glass", 0.5, 3.0, 15)
    with open(f"{base}/item/{c}_balloon.json", "w") as f:
        json.dump({"template": "minecraft:lead"}, f)
        f.write("\n")
block("obsidian_tnt", "minecraft:tnt", 0.0, 1200.0)
