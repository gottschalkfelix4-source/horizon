#!/usr/bin/env python3
"""Writes data/mekanism/horizon/{block,item}/*.json: the plain content of the port (templates are
placeholders until `horizon-port-pack carry` reads the real numbers). Run from this folder."""
import json, os

base = "data/mekanism/horizon"
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

ores = ["osmium", "tin", "lead", "uranium", "fluorite"]
for m in ores:
    block(f"{m}_ore", "minecraft:iron_ore", 3.0, 3.0)
    block(f"deepslate_{m}_ore", "minecraft:deepslate_iron_ore", 4.5, 3.0)
metals = ["osmium", "tin", "lead", "uranium"]
for m in metals:
    block(f"block_{m}", "minecraft:iron_block", 5.0, 6.0)
    block(f"block_raw_{m}", "minecraft:raw_iron_block", 5.0, 6.0)
    item(f"raw_{m}", "minecraft:raw_iron")
    for kind in ["ingot", "nugget"]:
        item(f"{kind}_{m}", "minecraft:iron_nugget" if kind == "nugget" else "minecraft:iron_ingot")
for m in metals + ["iron", "gold", "copper"]:
    for kind in ["dust", "clump", "shard", "crystal", "dirty_dust"]:
        item(f"{kind}_{m}", "minecraft:gunpowder")
for alloy in ["steel", "bronze", "refined_glowstone", "refined_obsidian"]:
    block(f"block_{alloy}", "minecraft:iron_block", 5.0, 6.0, 15 if alloy == "refined_glowstone" else None)
    item(f"ingot_{alloy}")
    item(f"nugget_{alloy}", "minecraft:iron_nugget")
    item(f"dust_{alloy}", "minecraft:gunpowder")
block("block_charcoal", "minecraft:coal_block", 5.0, 6.0)
for d in ["coal", "charcoal", "obsidian", "diamond", "lapis_lazuli", "quartz", "emerald", "sulfur", "salt", "lithium", "netherite"]:
    item(f"dust_{d}", "minecraft:gunpowder")
item("fluorite_gem", "minecraft:diamond")
item("yellow_cake_uranium")
for e in ["iron", "carbon", "redstone", "diamond", "glowstone", "obsidian", "tin", "gold", "copper"]:
    item(f"enriched_{e}")
for a in ["infused", "reinforced", "atomic"]:
    item(f"alloy_{a}")
for c in ["basic", "advanced", "elite", "ultimate"]:
    item(f"control_circuit_{c}")
    block(f"{c}_energy_cube", "minecraft:iron_block", 3.5, 16.0)
    block(f"{c}_universal_cable", "minecraft:iron_bars", 1.0, 6.0)
    for pipe in ["mechanical_pipe", "pressurized_tube", "logistical_transporter"]:
        block(f"{c}_{pipe}", "minecraft:iron_bars", 1.0, 6.0)
# the furnace template is what makes a click open the container's menu (as Powah's furnator)
for m in ["enrichment_chamber", "crusher", "energized_smelter", "precision_sawmill", "combiner", "metallurgic_infuser", "purification_chamber", "chemical_injection_chamber", "chemical_oxidizer"]:
    block(m, "minecraft:furnace", 3.5, 16.0)
for m in ["electrolytic_separator", "chemical_infuser", "isotopic_centrifuge"]:
    block(m, "minecraft:iron_block", 3.5, 16.0)
block("steel_casing", "minecraft:iron_block", 3.5, 9.0)
item("hdpe_pellet"); item("hdpe_sheet"); item("bio_fuel", "minecraft:wheat")

# factories: furnace template, as the machines they run
for t in ["basic", "advanced", "elite", "ultimate"]:
    for w in ["smelting", "enriching", "crushing", "sawing", "purifying", "injecting"]:
        block(f"{t}_{w}_factory", "minecraft:furnace", 3.5, 16.0)

# induction matrix: the port is a furnace-less plain block; cells and providers in four tiers
block("induction_casing", "minecraft:iron_block", 3.5, 9.0)
block("induction_port", "minecraft:iron_block", 3.5, 9.0)
for t in ["basic", "advanced", "elite", "ultimate"]:
    block(f"{t}_induction_cell", "minecraft:iron_block", 3.5, 9.0)
    block(f"{t}_induction_provider", "minecraft:iron_block", 3.5, 9.0)

block("dynamic_tank", "minecraft:iron_block", 3.5, 9.0)
block("dynamic_valve", "minecraft:iron_block", 3.5, 9.0)

for n in ["boiler_casing", "boiler_valve", "superheating_element", "turbine_casing", "turbine_valve",
          "turbine_rotor", "turbine_blade", "electromagnetic_coil"]:
    block(n, "minecraft:iron_block", 3.5, 9.0)

for n in ["fission_reactor_casing", "fission_reactor_port", "fission_fuel_assembly", "control_rod_assembly"]:
    block(n, "minecraft:iron_block", 3.5, 9.0)

for n in ["thermal_evaporation_block", "thermal_evaporation_valve", "thermal_evaporation_outlet"]:
    block(n, "minecraft:iron_block", 3.5, 9.0)
os.makedirs(f"{base}/fluid", exist_ok=True)
with open(f"{base}/fluid/brine.json", "w") as f:
    json.dump({"tint": 4290094079}, f)
    f.write("\n")

for n in ["fusion_reactor_frame", "fusion_reactor_port"]:
    block(n, "minecraft:iron_block", 3.5, 9.0)

for n in ["sps_casing", "sps_port"]:
    block(n, "minecraft:iron_block", 3.5, 9.0)
