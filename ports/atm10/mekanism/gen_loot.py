#!/usr/bin/env python3
"""Writes data/<ns>/loot_table/blocks/<block>.json for the Mekanism ports: every block drops itself
(ores drop their raw ore or gem; no silk touch or fortune yet). Run from ports/atm10:
`python3 mekanism/gen_loot.py`. Conditions use `type`, as 26.3 reads them."""
import json, os

PORTS = ["mekanism", "mekanismgenerators", "mekanismtools", "mekanismadditions"]
RAW = {"osmium", "tin", "lead", "uranium"}


def drop(ns, name):
    if name.endswith("_ore"):
        metal = name.removeprefix("deepslate_")[: -len("_ore")]
        if metal in RAW:
            return f"{ns}:raw_{metal}"
        if metal == "fluorite":
            return f"{ns}:fluorite_gem"
    return f"{ns}:{name}"


for ns in PORTS:
    blocks = os.path.join(ns, "data", ns, "horizon", "block")
    if not os.path.isdir(blocks):
        continue
    out = os.path.join(ns, "data", ns, "loot_table", "blocks")
    os.makedirs(out, exist_ok=True)
    count = 0
    for f in sorted(os.listdir(blocks)):
        if not f.endswith(".json"):
            continue
        name = f[:-5]
        table = {
            "type": "minecraft:block",
            "pools": [{
                "rolls": 1,
                "bonus_rolls": 0,
                "conditions": [{"type": "minecraft:survives_explosion"}],
                "entries": [{"type": "minecraft:item", "name": drop(ns, name)}],
            }],
            "random_sequence": f"{ns}:blocks/{name}",
        }
        with open(os.path.join(out, f), "w") as fh:
            json.dump(table, fh, indent=2)
            fh.write("\n")
        count += 1
    print(ns, count)
