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
