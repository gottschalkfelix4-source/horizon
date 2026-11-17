#!/usr/bin/env python3
"""Writes data/minecraft/tags/block/{mineable/pickaxe,needs_stone_tool,needs_iron_tool}.json for the
Mekanism ports (blocks that need a tool have no drops without them). Ores need an iron tool, every
other block a stone one. Run from ports/atm10: `python3 mekanism/gen_tags.py`."""
import json, os

PORTS = ["mekanism", "mekanismgenerators", "mekanismadditions"]

for ns in PORTS:
    blocks = os.path.join(ns, "data", ns, "horizon", "block")
    names = sorted(f[:-5] for f in os.listdir(blocks) if f.endswith(".json"))
    ores = [f"{ns}:{n}" for n in names if n.endswith("_ore")]
    rest = [f"{ns}:{n}" for n in names if not n.endswith("_ore")]
    tags = {
        "mineable/pickaxe": ores + rest,
        "needs_iron_tool": ores,
        "needs_stone_tool": rest,
    }
    for tag, values in tags.items():
        path = os.path.join(ns, "data", "minecraft", "tags", "block", tag + ".json")
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w") as f:
            json.dump({"replace": False, "values": values}, f, indent=2)
            f.write("\n")
    print(ns, len(names))
