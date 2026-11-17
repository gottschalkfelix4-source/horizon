"""Shared by the gen_data.py of the Mystical Agradditions, Create Additions and Just Dire Things
ports (identical copy in each folder): turns the original's data files (1.21.1 formats, MIT) into
26.3 ones, mirroring crates/horizon-port-pack/src/carry.rs (`ingredient`, `upgrade_recipe`,
`upgrade_loot`). Only the original's data files are read, never its code."""
import json
import os
import re

ID = re.compile(r"^[a-z0-9_.-]+:[a-z0-9_./-]+$")


def read(path):
    with open(path) as f:
        return json.load(f)


def write(path, value):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        json.dump(value, f, indent=2, sort_keys=False)
        f.write("\n")


def files(root, sub):
    """Every json under root/sub, as (relative path without .json, parsed value); `._*` files skipped."""
    base = os.path.join(root, sub)
    for dirpath, _, names in os.walk(base):
        for n in sorted(names):
            if n.startswith("._") or not n.endswith(".json"):
                continue
            full = os.path.join(dirpath, n)
            yield os.path.relpath(full, base)[:-5], read(full)


# ---------------------------------------------------------------------------- ingredients

def ingredient(v):
    """A 1.21.1 ingredient as 26.3's (`"id"`, `"#tag"`, a list of ids); None when it has no 26.3 form."""
    if isinstance(v, str):
        return v
    if isinstance(v, dict) and len(v) == 1:
        if isinstance(v.get("item"), str):
            return v["item"]
        if isinstance(v.get("tag"), str):
            return "#" + v["tag"]
        return None
    if isinstance(v, list):
        out = []
        for x in v:
            if not (isinstance(x, dict) and len(x) == 1 and isinstance(x.get("item"), str)):
                return None
            out.append(x["item"])
        return out[0] if len(out) == 1 else out
    return None


def ids_in(v):
    """Every item id and tag a converted recipe names (ingredients and the result)."""
    if isinstance(v, str):
        if ID.match(v):
            yield v
        elif v.startswith("#") and ID.match(v[1:]):
            yield v
    elif isinstance(v, list):
        for x in v:
            yield from ids_in(x)


def upgrade_recipe(recipe):
    """The recipe in 26.3's shape, or None when its type or an ingredient has none here."""
    kind = recipe.get("type", "")
    if kind == "cucumber:shaped_no_mirror":
        kind = "crafting_shaped"
    if kind.startswith("minecraft:"):
        kind = kind[len("minecraft:"):]
    known = {"crafting_shaped", "crafting_shapeless", "smelting", "blasting", "smoking",
             "campfire_cooking", "smithing_transform", "smithing_trim", "stonecutting"}
    if kind not in known:
        return None
    out = dict(recipe)
    out["type"] = "minecraft:" + kind
    if "key" in out:
        out["key"] = {}
        for k, v in recipe["key"].items():
            new = ingredient(v)
            if new is None:
                return None
            out["key"][k] = new
    if "ingredients" in out:
        out["ingredients"] = []
        for v in recipe["ingredients"]:
            new = ingredient(v)
            if new is None:
                return None
            out["ingredients"].append(new)
    for f in ("ingredient", "base", "addition", "template"):
        if f in out:
            new = ingredient(recipe[f])
            if new is None:
                return None
            out[f] = new
    return out


def recipe_ids(recipe):
    """The ids a converted recipe needs: ingredients, then its result."""
    needs = []
    for f in ("ingredient", "base", "addition", "template"):
        if f in recipe:
            needs += list(ids_in(recipe[f]))
    for v in recipe.get("key", {}).values():
        needs += list(ids_in(v))
    for v in recipe.get("ingredients", []):
        needs += list(ids_in(v))
    result = recipe.get("result")
    res = result.get("id") if isinstance(result, dict) else result
    return needs, res


def condition_holds(conds, item_exists, crops_enabled=lambda c: True):
    """Evaluates the conditions only this port can answer. Returns (holds, remaining) where
    remaining are the ones left for the engine (`neoforge:mod_loaded` and the like)."""
    keep = []
    for c in conds:
        t = c.get("type")
        if t == "neoforge:item_exists":
            if not item_exists(c["item"]):
                return False, []
        elif t == "mysticalagriculture:crop_enabled":
            if not crops_enabled(c["crop"]):
                return False, []
        else:
            keep.append(c)
    return True, keep


# ---------------------------------------------------------------------------- loot tables

def _type_number(n):
    if isinstance(n, dict):
        if "type" not in n and "min" in n and "max" in n:
            n["type"] = "minecraft:uniform"
        t = n.get("type", "").removeprefix("minecraft:")
        keys = {"uniform": ["min", "max"], "binomial": ["n", "p"]}.get(t, [])
        for k in keys:
            if k in n:
                _type_number(n[k])


def _named_tool(pred):
    keys = list(pred.keys())
    if keys == ["predicates"]:
        lst = pred["predicates"].get("minecraft:enchantments")
        if isinstance(lst, list) and len(lst) == 1 and len(pred["predicates"]) == 1:
            e = lst[0]
            if e.get("enchantments") == "minecraft:silk_touch" and e.get("levels", {}).get("min", 1) <= 1 \
                    and "max" not in e.get("levels", {}):
                return "minecraft:tool/can_silk_touch"
    if keys == ["items"]:
        if pred["items"] in ("minecraft:shears", ["minecraft:shears"]):
            return "minecraft:tool/can_shear"
    return None


def _rename(v, key):
    if isinstance(v, dict) and "type" not in v and key in v:
        # keep `type` first
        kind = v.pop(key)
        rest = dict(v)
        v.clear()
        v["type"] = kind
        v.update(rest)


def _cond(c):
    _rename(c, "condition")
    if "term" in c:
        _cond(c["term"])
    for t in c.get("terms", []):
        _cond(t)
    if c.get("type") == "minecraft:match_tool":
        named = _named_tool(c.get("predicate", {}))
        if named:
            return named
    return c


def upgrade_loot(v):
    if isinstance(v, list):
        return [upgrade_loot(x) for x in v]
    if not isinstance(v, dict):
        return v
    o = dict(v)
    for pool in o.get("pools", []) if isinstance(o.get("pools"), list) else []:
        for k in ("rolls", "bonus_rolls"):
            if k in pool:
                _type_number(pool[k])
    if isinstance(o.get("conditions"), list):
        lst = [_cond(c) for c in o.pop("conditions")]
        o["condition"] = lst[0] if len(lst) == 1 else {"type": "minecraft:all_of", "terms": lst}
    if isinstance(o.get("functions"), list):
        lst = o.pop("functions")
        for f in lst:
            _rename(f, "function")
            if isinstance(f.get("count"), dict):
                _type_number(f["count"])
            upgrade_loot_in_place(f)
        o["modifier"] = lst
    for k, x in list(o.items()):
        if k not in ("condition", "modifier"):
            o[k] = upgrade_loot(x)
    return o


def upgrade_loot_in_place(f):
    for k, x in list(f.items()):
        if k == "conditions" and isinstance(x, list):
            lst = [_cond(c) for c in f.pop("conditions")]
            f["condition"] = lst[0] if len(lst) == 1 else {"type": "minecraft:all_of", "terms": lst}
        elif isinstance(x, (dict, list)) and k not in ("condition",):
            f[k] = upgrade_loot(x)


# ---------------------------------------------------------------------------- tags

def tag_values(values, exists):
    """A tag's values with ids of this mod that do not exist dropped and other mods' ids optional."""
    out = []
    for v in values:
        vid = v["id"] if isinstance(v, dict) else v
        if vid.startswith("#"):
            out.append(v)
        elif exists(vid):
            out.append(v)
        elif not vid.startswith("minecraft:") and vid.split(":")[0] not in OWN:
            out.append({"id": vid, "required": False} if not isinstance(v, dict) else v)
    return out


OWN = set()
