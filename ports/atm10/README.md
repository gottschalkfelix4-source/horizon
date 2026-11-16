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
