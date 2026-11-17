#!/usr/bin/env python3
"""Writes data/mekanismtools/horizon/item/*.json: tools and armor over vanilla's iron templates,
with each material's durability and damage (placeholders from memory, listed in README.md under
"To check"; textures and equipment models are iron's until `horizon-port-pack carry`)."""
import json, os

d = "data/mekanismtools/horizon/item"
os.makedirs(d, exist_ok=True)

# name: (tool durability, damage bonus, armor durability factor, armor points [boots, legs, chest, helmet], toughness, knockback resistance)
MATERIALS = {
    "bronze": (800, 2.0, 30, (3, 6, 7, 3), 1.0, 0.0),
    "lapis_lazuli": (200, 1.0, 12, (2, 5, 6, 2), 0.0, 0.0),
    "osmium": (1000, 3.0, 36, (3, 6, 8, 3), 1.0, 0.0),
    "refined_glowstone": (1100, 2.0, 33, (3, 6, 8, 3), 2.0, 0.0),
    "refined_obsidian": (3000, 8.0, 80, (5, 8, 10, 5), 5.0, 0.2),
    "steel": (1400, 3.0, 40, (3, 6, 8, 3), 2.0, 0.0),
}
# tool: (damage over the material's bonus, attack speed offset)
TOOLS = {"sword": (3.0, -2.4), "pickaxe": (1.0, -2.8), "axe": (6.0, -3.1), "shovel": (1.5, -3.0), "hoe": (0.0, -1.0)}
ARMOR = {"boots": (0, 13), "leggings": (1, 15), "chestplate": (2, 16), "helmet": (3, 11)}
SLOT = {"boots": "feet", "leggings": "legs", "chestplate": "chest", "helmet": "head"}


def write(name, obj):
    with open(f"{d}/{name}.json", "w") as f:
        json.dump(obj, f, indent=2)
        f.write("\n")


for m, (dur, bonus, factor, points, tough, kb) in MATERIALS.items():
    for t, (base, speed) in TOOLS.items():
        write(f"{m}_{t}", {"template": f"minecraft:iron_{t}", "components": {
            "minecraft:max_damage": dur,
            "minecraft:attribute_modifiers": [
                {"type": "minecraft:attack_damage", "id": "minecraft:base_attack_damage",
                 "amount": base + bonus, "operation": "add_value", "slot": "mainhand"},
                {"type": "minecraft:attack_speed", "id": "minecraft:base_attack_speed",
                 "amount": speed, "operation": "add_value", "slot": "mainhand"}]}})
    for a, (i, mult) in ARMOR.items():
        mods = [
            {"type": "minecraft:armor", "id": f"mekanismtools:{m}_{a}", "amount": points[i],
             "operation": "add_value", "slot": SLOT[a]},
            {"type": "minecraft:armor_toughness", "id": f"mekanismtools:{m}_{a}", "amount": tough,
             "operation": "add_value", "slot": SLOT[a]}]
        if kb:
            mods.append({"type": "minecraft:knockback_resistance", "id": f"mekanismtools:{m}_{a}",
                         "amount": kb, "operation": "add_value", "slot": SLOT[a]})
        write(f"{m}_{a}", {"template": f"minecraft:iron_{a}", "components": {
            "minecraft:max_damage": factor * mult, "minecraft:attribute_modifiers": mods}})
