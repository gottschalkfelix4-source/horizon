#!/usr/bin/env python3
"""Writes assets/<ns>/lang/en_us.json for the Mekanism ports from the ids in their data folders:
`python3 mekanism/gen_lang.py` from ports/atm10. Names follow Mekanism's: "Iron Dust", "Block of Osmium"."""
import json, os, sys

PORTS = {"mekanism": "mekanism", "mekanismgenerators": "mekanismgenerators",
         "mekanismtools": "mekanismtools", "mekanismadditions": "mekanismadditions"}
FORMS = {"dust": "Dust", "clump": "Clump", "shard": "Shard", "crystal": "Crystal", "dirty_dust": "Dirty Dust",
         "ingot": "Ingot", "nugget": "Nugget", "raw": "Raw", "enriched": "Enriched"}
SMALL = {"of", "the"}


def title(words):
    return " ".join(w.capitalize() if w not in SMALL else w for w in words.split("_"))


def name(path):
    for form in sorted(FORMS, key=len, reverse=True):
        if path.startswith(form + "_"):
            rest = title(path[len(form) + 1:])
            if form in ("raw", "enriched"):
                return f"{FORMS[form]} {rest}"
            if form == "block":
                break
            return f"{rest} {FORMS[form]}"
    if path.startswith("block_raw_"):
        return f"Block of Raw {title(path[10:])}"
    if path.startswith("block_"):
        return f"Block of {title(path[6:])}"
    if path.startswith("alloy_"):
        return f"{title(path[6:])} Alloy"
    if path.startswith("control_circuit_"):
        return f"{title(path[16:])} Control Circuit"
    return title(path)


for port, ns in PORTS.items():
    data = os.path.join(port, "data", ns, "horizon")
    lang = {}
    for kind in ("block", "item"):
        d = os.path.join(data, kind)
        if not os.path.isdir(d):
            continue
        for f in sorted(os.listdir(d)):
            if f.endswith(".json"):
                lang[f"{kind}.{ns}.{f[:-5]}"] = name(f[:-5])
                if kind == "item":
                    # the game names a content item by its block key too in places
                    lang.setdefault(f"block.{ns}.{f[:-5]}", name(f[:-5]))
    out = os.path.join(port, "assets", ns, "lang")
    os.makedirs(out, exist_ok=True)
    with open(os.path.join(out, "en_us.json"), "w") as fh:
        json.dump(lang, fh, indent=2, ensure_ascii=False)
        fh.write("\n")
    print(port, len(lang))
