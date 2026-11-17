# ATM10 ports

Native Horizon ports of All the Mods 10 (8.6, Minecraft 1.21.1, NeoForge) mods, one folder per mod.
Each is a native mod crate (`horizon-mod.toml`, `Cargo.toml`, `src/`) built against this checkout's
`crates/horizon-mod-api`. The method and the rules are Horizon's own: [docs/modpacks.md](../../docs/modpacks.md).

Rules followed here:

- No mod's code is decompiled, read or quoted. Ports are written from observed behaviour, the mod's
  data files and its public documentation.
- Only mods whose licence allows publishing a derivative (permissive, or copyleft with this
  port under the same licence) are committed. Other ports stay on the machine that made them.
- A gap in Horizon's mod API is filled in the engine, in its own commit, so it can go upstream.

| Mod | ATM10 version | Licence | Tier | Status |
|---|---|---|---|---|
| [Libraries](libs/) | | LGPL-3.0-or-later | libs | config, geo (GeckoLib), accessories (Curios), guidebook (Patchouli): data sides done, see libs/README.md |
| [Torchmaster](torchmaster/) | 21.1.13 | MIT | logic | Mega Torch and Dread Lamp ported; Feral Flare Lantern and config to do; not yet checked against the original |
| [AppleSkin](appleskin/) | 3.0.9+mc1.21 | Unlicense | client | ported in part; not run in the game, not yet checked against the original |
| [Auroras](auroras/) | 1.6.2 | BSD-2-Clause | client | ported in part; not run in the game, not yet checked against the original |
| [Better Advanced Tooltips](betteradvancedtooltips/) | 2101.1.0-build.5 | MIT | client | ported in part; not run in the game, not yet checked against the original |
| [Colorful Hearts](colorfulhearts/) | 10.5.9 | MIT | client | ported in part; not run in the game, not yet checked against the original |
| [Controlling](controlling/) | 19.0.5 | MIT | client | ported in part; not run in the game, not yet checked against the original |
| [Dark Mode Everywhere](darkmodeeverywhere/) | 1.21.1-1.4.0 | MIT | client | ported in part; not run in the game, not yet checked against the original |
| [EnchantmentDescriptions](enchdesc/) | 21.1.11 | LGPL-2.1-only | client | ported in part; not run in the game, not yet checked against the original |
| [Extreme Sound Muffler](extremesoundmuffler/) | 3.57 | LGPL-3.0-only | client | ported in part; not run in the game, not yet checked against the original |
| [Just Enough Items](jei/) | 19.64.0.454 | MIT | client | ported in part; not run in the game, not yet checked against the original |
| [Just Enough Breeding](justenoughbreeding/) | 3.3.1 | MIT | client | ported in part; not run in the game, not yet checked against the original |
| [Just Enough Professions (JEP)](justenoughprofessions/) | 4.0.5 | MIT | client | ported in part; not run in the game, not yet checked against the original |
| [KeyBind Bundles](keybindbundles/) | 1.4.0 | MIT | client | ported in part; not run in the game, not yet checked against the original |
| [KeybindsPurger](keybindspurger/) | 1.4.0 | MIT | client | ported in part; not run in the game, not yet checked against the original |
| [More Overlays Updated](moreoverlays/) | 1.24.2 | MIT | client | ported in part; not run in the game, not yet checked against the original |
| [Polymorph](polymorph/) | 1.2.0+1.21.1 | LGPL-3.0-or-later | client | ported in part; not run in the game, not yet checked against the original |
| [SmithingTemplateViewer](smithingtemplateviewer/) | 1.0.4 | MIT | client | ported in part; not run in the game, not yet checked against the original |
| [Toast Control](toastcontrol/) | 9.0.1 | MIT | client | ported in part; not run in the game, not yet checked against the original |
| [Luminax](luminax/) | 1.3.1 | LGPL-3.0-or-later | client | README only: behaviour must be observed first |
| [Just Enough Mekanism Multiblocks](jei_mekanism_multiblocks/) | 7.23 | MIT | client | README only: waits for the Mekanism port |
| [Refined Storage - JEI Integration](refinedstorage_jei_integration/) | 1.0.0 | MIT | client | README only: waits for the Refined Storage port |
| [Mouse Tweaks](mousetweaks/) | 2.26.1 | BSD-3-Clause | client | covered by Horizon's built-in extras; gaps listed |
| [Accelerated Decay](accelerateddecay/) | 21.0.0 | LGPL-3.0-only | logic | leaves decay a few per tick; queue not saved |
| [Apothic Spawners](apothic_spawners/) | | MIT | logic | modifier items on vanilla spawners; needs a spawner tick hook for some switches |
| [Baubley Heart Canisters](bhc/) | | MIT | logic | hearts, canisters, amulet; no accessory slot in the API, first amulet in the inventory counts |
| [Bridging Mod](bridgingmod/) | | MIT | logic | outline only; placing needs a client use-item call |
| [Bad Wither No Cookie Reloaded](bwncr/) | | MIT | logic | sound cancelling and commands; world-event sounds need a hook |
| [Comforts](comforts/) | | LGPL-3.0-or-later | logic | sleeping bags and hammocks; hammock pairing not yet |
| [Construction Sticks](constructionstick/) | | MIT | logic | five sticks, grow-surface mode; other modes not yet |
| [Crash Utilities](crashutilities/) | | MIT | logic | `/cu` commands; block entity and offline inventory features need API |
| [ElevatorMod](elevatorid/) | | MIT | logic | 16 elevators; disguise not yet |
| [I'm Fast](imfast/) | | MIT | logic | movement checks off through game rules |
| [InControl](incontrol/) | | MIT | logic | spawn and experience rules; loot, despawn and actions not yet |
| [Iron Jetpacks](ironjetpacks/) | | MIT | logic | jetpack with tiers, hover, energy; recipes and HUD not yet |
| [Measurements](measurements/) | | MIT | logic | tape measure; edge labels need world text |
| [Mama's MerryMaking](merrymaking/) | | MIT | logic | menorah and kinara only; public docs describe little else |
| [More Red](morered/) | | MIT | logic | red alloy wire and 14 gates; weak power only, bundled cables not yet |
| [No Villager Death Messages](novillagerdm/) | | MIT | logic | nothing to suppress in Horizon's server |
| [Redstone Pen](redstonepen/) | | MIT | logic | pen, quill, track and four relays |
| [Repeatable Trial Vaults](repeatable_trial_vaults/) | | MIT | logic | key items reset a vault for a rewarded player |
| [Restrictions](restrictions/) | | MIT | logic | Attractor, Pusher, One Way |
| [Silent's Gems](silentgems/) | | MIT | logic | gems, teleporters, linker, charm; worldgen and recipes from the jar |
| [Simple Backups](simplebackups/) | | Apache-2.0 | logic | timed zip backups and `/backup` |
| [Time In A Bottle](tiab/) | | MIT | logic | stored time and block acceleration; block entities need a tick call |
| [WITS](wits/) | | MIT | logic | `/wits` and `/witsop` for vanilla structures |
| [Wither Skeleton Tweaks](wstweaks/) | | MIT | logic | Nether skeleton conversion, fragments, Immolation Blade |

All of these were built and unit-tested only (`cargo test`, clippy, fmt); none has run in the game or been checked against the original. Each README lists what is assumed and what Horizon's API still lacks.
| [Iron Furnaces](ironfurnaces/) | 4.3.2 | Apache-2.0 | machines | 13 tiers smelt with vanilla's furnace tick and menu; augments, upgrades and XP to do; not yet checked against the original |
| [Industrial Foregoing](industrialforegoing/) | 1.21-3.6.39 | MIT | machines | Latex chain, Dissolution Chamber, Mob Crusher, Plant Gatherer/Sower, Pitiful Generator and more ported; not yet checked against the original |
| [Farmer's Delight](farmersdelight/) | 1.3.4 | MIT | machines | Cooking Pot, heat sources, Stove, Skillet, Cutting Board, Rich Soil ported (vanilla 3x3 menu as stand-in); not yet checked against the original |
| [Powah](powah/) | 6.2.10 | LGPL-3.0-only | machines | Energy cells and cables, Furnator, Magmator, Thermo, Solar and Reactor ported; not yet checked against the original |
| [Applied Energistics 2](ae2/) | 19.2.18 | LGPL-3.0-or-later | deep | stage 1: cables, controller, energy cells, network power; later stages not yet; not run in the game |
| [Mekanism](mekanism/) | 10.7.0 | MIT | machines | First slice: ore and metal items, energy cubes, universal cables, enrichment chamber, crusher, energized smelter, `mekanism:chemicals` capability; generators, tools, additions, factories, multiblocks to do; not yet checked against the original |
| [Mekanism Generators](mekanismgenerators/) | 10.7.0 | MIT | machines | Solar, advanced solar, wind, bio and heat (items only) generators ported; gas generator and multiblock generators to do; not yet checked against the original |
| [Modern Dynamics](moderndynamics/) | 0.9.6 | LGPL-3.0-only | machines | Item and fluid pipes with servos, energy cables ported; visible item travel, shapes and other attachments to do |
| [Modular Routers](modularrouters/) | 13.2.7 | MIT | machines | Router run cycle, speed/stack/range upgrades and the sender modules ported; remaining modules to do |
| [Transfer Labels](transfer_labels/) | 0.1.9 | MIT | machines | Extract/insert labels with named channels transfer items; filters, fluid/energy labels to do |
| [EnderStorage](enderstorage/) | 2.13.0.191 | MIT | machines | Ender Chest, Ender Tank (shared by 3-colour frequency, personal via diamond) and Ender Pouch ported; recipes and models to do; not yet checked against the original |
| [RFToolsBase](rftoolsbase/) | 1.21-6.0.11 | MIT | machines | Filter Module rules and Information Screen ported; shard ores, filter GUI and display to do; not yet checked against the original |
| [RFToolsPower](rftoolspower/) | 1.21-7.0.6 | MIT | machines | Cells, Coal and Endergenic generators, Pearl Injector and Power Monitor ported; GUIs and models to do; not yet checked against the original |
| [RFToolsStorage](rftoolsstorage/) | 1.21-6.0.5 | MIT | machines | Modular Storage with three module tiers ported; Remote module and Storage Scanner to do; not yet checked against the original |
| [RFToolsUtility](rftoolsutility/) | 1.21-7.0.12 | MIT | machines | Timer, Counter, Redstone Transmitter/Receiver and Environmental Controller ported; crafters, spawner, matter transmitter, screens to do; not yet checked against the original |
| [XNet](xnet/) | 1.21-7.0.7 | MIT | machines | Cable network, connectors and controller moving items, fluids and energy by channel ported; routers, filters, facades to do; not yet checked against the original |
| [Industrial Foregoing Souls](industrialforegoingsouls/) | 1.10.7 | MIT | machines | Low confidence: Soul Surge stand-in only; the mod's real blocks not recalled |
| [More Industrial Foregoing Addons](mifa/) | 2.1.0 | MIT | machines | Low confidence: Lava Generator stand-in only; the mod's real blocks not recalled |
| [Ranged Pumps](rangedpumps/) | 1.3.0 | MIT | machines | Ranged Pump (range scan, energy, tank) ported with assumed numbers; further tiers to do |
| [QuarryPlus](quarryplus/) | 21.1.164 | LGPL-3.0-only | machines | Marker-defined Quarry digging by layer for energy ported; frame, pump, enchantments and other machines to do |
| [DimStorage](dimstorage/) | 9.1.1 | AGPL-3.0-only | machines | Dimensional Chest, Tank and Tablet shared by numbered frequency with owner lock ported; Controller and frequency screen to do |
| [Step Crafter](stepcrafter/) | 1.21.1-0.1.9 | MIT | machines | Redstone-pulse 3x3 crafter ported; recipe preview and comparator to do |
| [Actually Additions](actuallyadditions/) | 1.3.26 | MIT | machines | Coal generator, powered furnace and crusher ported (crusher recipes hard-coded); rest of the mod to do |
| [Extreme Reactors](bigreactors/) | 1.21.1-2.4.28 | MIT | machines | Passive multiblock reactor with the port's own numbers; turbines and active cooling to do |
| [Laser Bridges](laserbridges/) | 6.2 | MIT | machines | Emitter, beam and receiver as an assumed reading of the mod |
| [Interdimensional Wireless Transmitter](interdimensionalwirelesstransmitter/) | 1.21.1-0.1.5 | MIT | machines | Placeholder: redstone over frequencies across dimensions; an assumption about the original |
| [Functional Storage](functionalstorage/) | 1.21.1-1.5.8 | MIT | machines | Plain drawers (1/2/4 slots, six woods) ported; compacting drawers, controller, upgrades to do |
| [Pocket Storage](pocketstorage/) | 1.2.5 | MIT | machines | Placeholder: three storage blocks with sort; original's content not recalled |
| [Lootr](lootr/) | 1.21.1-1.11.38.127 | MIT | machines | Per-player loot chest ported (one viewer at a time); vanilla loot containers cannot be converted yet |
| [Bibliocraft Legacy](bibliocraft/) | 1.6.5 | MIT | machines | Bookcase and potion shelf ported; rest of the furniture mod to do |
| [Hostile Neural Networks](hostilenetworks/) | 6.5.1 | MIT | machines | Data models, simulation chamber and loot fabricator for three mobs; numbers assumed |
| [Apothic Enchanting](apothic_enchanting/) | 1.6.4 | MIT | machines | Four shelves feeding table power and a book library; levels above 30 blocked by the engine's power cap |
| [BotanyPots](botanypots/) | 21.1.44 | LGPL-2.1-only | machines | Botany pot with 9 soils and 11 crops (not data-driven) ported |
| [Mystical Agriculture](mysticalagriculture/) | 8.0.28 | MIT | machines | 16 essence crops and five farmland tiers with tier-gated growth; rest to do |
| [Storage Delight](storagedelight/) | 26.09.17-1.21-neoforge | MIT | machines | Placeholder: food-only pantry and fridge; original's blocks not recalled |
| [Sushi Go Crafting](sushigocrafting/) | 0.6.6 | MIT | machines | Rolling mat making rolls (assumed); ingredients not obtainable yet |

## Building

Each port pins its build in `horizon-mod.toml` (`[native]`: `game`, `horizon`, `mixins`) and names the SDK
by the path the launcher takes, `.horizon/sdk/api/crates/horizon-mod-api`. `horizon` is the fork commit
that carries the engine's `Mod API:` commits the ports need; move it when a later one is needed. In a
checkout, run `ports/atm10/dev-sdk.sh` once: it links each port's `.horizon/sdk/api` to this repository's
crates, then `cargo check` and `cargo test` work in a port folder. `justenoughbreeding` and
`justenoughprofessions` get `jei` from the launcher, so they build through it.
