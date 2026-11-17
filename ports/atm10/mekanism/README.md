# Mekanism (Horizon port)

Port of [Mekanism](https://github.com/mekanism/Mekanism) 10.7 (ATM10 8.x, MIT) as a native Horizon
mod. Written from the mod's observed behaviour and public description, not from its code.
Mekanism is a large mod (Core, Generators, Tools, Additions); this is the first slice, built so
the later ones can sit on it.

## What it does

| Part | Behaviour | Status |
|---|---|---|
| Ores and metals | Osmium, tin, lead, uranium, fluorite ores (stone and deepslate), raw ores, ingots, nuggets, dusts, clumps, shards, crystals, dirty dusts, storage blocks, steel, bronze, refined glowstone/obsidian, enriched items, alloys, circuits | blocks and items declared (placeholders for look and numbers until `carry`); no world generation, loot tables or crafting recipes yet |
| Energy cubes | Four tiers (2, 8, 32, 128 MJ at 2.5 J per FE); push into adjacent machines, cables draw from them | ported, numbers to check |
| Universal cables | Four tiers; a network (up to 4096 cables) run by its lowest cable moves energy from cubes it touches to machines it touches, at the lowest tier's rate | ported, network model is the port's own (Mekanism's transmitters hold a buffer and balance) |
| Enrichment chamber | 50 J/tick, 200 ticks: ores to two dusts, fluorite ore to six gems, redstone/coal/diamond/glowstone/iron dust to enriched items | ported, recipe table hard-coded |
| Crusher | Ingots to dusts, cobblestone to gravel to sand, bones, blaze rods, gems to dusts | ported, recipe table hard-coded |
| Energized smelter | The engine's `minecraft:smelting` recipes, same power and time | ported |
| Precision sawmill | Logs and stems to six planks (no sawdust chance yet) | ported |
| Combiner | Dust and cobblestone into the metal's ore | ported |
| Metallurgic infuser | Infusing items in the second slot fill an infuse tank (10 per item, 1000 max); makes infused/reinforced/atomic alloys, steel and bronze dust | ported, recipe table hard-coded |
| Purification chamber, chemical injection chamber | An ore into 3 clumps with oxygen (raw: 2, shard: 1); an ore into 4 shards with hydrogen chloride (raw: 3); 1 mB per tick; clumps crush to dirty dusts, dirty dusts enrich to dusts | ported, tables hard-coded |
| Electrolytic separator, chemical infuser | Water (or brine) splits into hydrogen and oxygen (chlorine and sodium); hydrogen and chlorine join into hydrogen chloride; tanks reached through `mekanism:chemicals` | ported without screens; brine has no source until the thermal evaporation plant exists |
| Factories | Basic/advanced/elite/ultimate (3, 5, 7, 9 lanes) smelting, enriching, crushing, sawing, purifying and injecting factories: each lane runs its machine's recipe, all sharing one energy buffer (lanes x the machine's) and one chemical tank; 3x3-row chest menu (inputs, then outputs, energy item, upgrades) | ported; no combining/infusing/compressing factories yet, upgrades and tier-up installation not yet |
| Induction matrix | A hollow box (3 to 18 per side) of induction casing and ports around induction cells (1/8/64/512 GJ) and providers (rate), air allowed inside; every port is an energy block for the whole structure (re-checked once a second; the lowest port keeps the energy) | ported; energy handlers are 32-bit, so a matrix over about 4.3 billion FE shows as full to cables and the stored energy is lost when the structure breaks; no screen |
| Dynamic tank | A hollow box (3 to 18 per side) of dynamic tank casing and valves, empty inside, 16 000 mB per interior block of one fluid or one chemical; every valve is a fluid and a chemical capability for the whole structure (lowest valve keeps the contents; re-checked once a second) | ported; no screen, no comparator output, contents are lost when the structure breaks |
| Boiler | Hollow box of boiler casing and valves around superheating elements: water in (fluid pipe), energy in (cable), steam out (tube); 500 mB water per element per tick at 4 FE per mB | stand-in: Mekanism heats it from fission reactors or heaters, which are not ported, so the elements are electric |
| Turbine | Hollow box of turbine casing and valves around turbine rotor, blades and electromagnetic coils: steam in (tube), energy out (cable); up to 2 000 mB steam per blade per tick for 4 FE per mB, scaled down below one coil per two blades | stand-in formula; no rotor-length, vent or condenser rules, no screen |
| Fission reactor | Hollow box of reactor casing and ports around fission fuel assemblies and control rod assemblies: fissile fuel in (tube), water in (pipe), steam and nuclear waste out (tube); each assembly burns up to 1 mB fuel a tick, 1 mB fuel turns 1 000 mB water into steam, nothing runs without a rod | stand-in: no heat, damage or meltdown, no logic adapter; ids are `mekanism:` (the original's are `mekanismgenerators:`) because the chemical capability is typed by this crate; no fuel source until the nuclear chain (oxidizer, centrifuge, ...) is ported |
| Chemical oxidizer, isotopic centrifuge | Uranium dust/ingot/yellowcake into uranium oxide, fluorite into hydrogen fluoride (250 mB per item, 200 ticks); chemical infuser joins them into uranium hexafluoride; the centrifuge turns that into fissile fuel (5 mB a tick) for the fission reactor | stand-in chain (the original's has more steps and by-products) |
| Thermal evaporation plant | Hollow box of evaporation blocks and valves, empty inside: water in (pipe, tank 0), brine out (tank 1, the new `mekanism:brine` fluid); with open sky over the middle of the roof 10 mB of water a tick per block of inside height | stand-in: the original needs a 4x4 base and heat and loses efficiency; no screen |
| Fusion reactor | Hollow box of fusion reactor frames and ports, empty inside: deuterium and tritium in (tubes), energy out; up to 10 mB of each a tick fuse into 20 000 FE per pair. The isotopic centrifuge makes deuterium from hydrogen and tritium from lithium | stand-in: no laser, plasma heat, injection rate or blanket; the fuel routes are made up |
| Mechanical pipes, pressurized tubes, logistical transporters | Four tiers each; a network (up to 4096 pipes, run by its lowest) moves fluids (1, 4, 16, 64 B/tick), chemicals (same) or items (1, 2, 4, 8 per tick) between the blocks touching it, every one both giving and taking | ported, simplified: no side configuration, filters, colours or routing, and two chests joined by a transporter swap items back and forth |
| `mekanism:chemicals` | `CapabilityKey` over `ResourceHandler<ChemicalResource>`, a one-tank block handler saved in the block entity | declared and unit-tested; no machine registers it yet |
| Menus | Vanilla hopper menu (slots: input, output, energy item, two upgrades) | stand-in until mod menus with energy bars exist |
| Generators, tools, additions | in the [Generators](../mekanismgenerators/), [Tools](../mekanismtools/) and [Additions](../mekanismadditions/) ports | | |
| Factories, tiers of machines, upgrades, side configuration, security, QIO, teleporters, digital miner, multiblocks (generators, turbine, fission, fusion, SPS), tools and armor, Additions, gas machines, pipes (pressurized, logistical, mechanical, thermodynamic), jetpack/mekasuit, chemical/fluid processing | | not yet |

## Needs from Horizon

Reported to the API thread, which builds them (see `../ENGINE-GAPS.md`):

1. Mod menu widgets (energy bar, progress arrow, number fields) for machine screens.
2. Per-slot rules for a container so hoppers do not push into the output or the energy slot.
3. Item-backed container menus (the portable QIO dashboard, the configurator, the Mekasuit).
4. `ChunkRef::block_entities`, for multiblocks and the digital miner scanning a chunk.
5. A data-driven recipe type with several inputs, fluids and chemicals so `mekanism:enriching`,
   `:crushing`, `:infusing`, `:dissolution` and the rest come from datapacks (as `LevelRef::recipe_for` does
   for crafting kinds); until then the tables are in `src/rules.rs`.

Not a gap: gases, infuse types, pigments and slurries need no engine change (`CapabilityKey::new`).

## To check against the NeoForge original

1. Every number in `src/rules.rs` (cube capacity and rate, cable rate, machine power, time and buffer).
2. Whether raw ores enrich to two dusts and ores to two (as written), and the fluorite yield.
3. The cube's face configuration (here: all faces in and out).
4. Hardness and resistance (`gen_data.py` writes placeholders until `horizon-port-pack carry`).

## Build

```sh
sh ../dev-sdk.sh                              # once: links the checkout's mod API
cargo test                                    # in this folder
python3 gen_data.py                           # rewrites data/ (placeholders)
python3 gen_lang.py                           # from ports/atm10: names for all four Mekanism ports
horizon-launcher native-build . --instance <instance> --trust-native-code
```
