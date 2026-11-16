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
