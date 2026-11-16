# ATM10 libraries on Horizon

Libraries that ATM10's mods share, rebuilt as crates on Horizon's mod API. A ported mod depends on
the one it needs by path (`../libs/<name>`). They are written from public documentation and
observed behaviour, never from the libraries' code.

| Crate | Replaces | What it gives | Status |
|---|---|---|---|
| `config` | NeoForge `ModConfigSpec`, Configuration, Cloth Config | typed schema, TOML file with comments and ranges, correction on load | done, unit-tested |
| `geo` | GeckoLib | `.geo.json` models, `.animation.json` with Molang and easings, entity renderer with a controller | done, unit-tested; not yet seen in the game |
| `accessories` | Curios | accessory slots | to do |
| `guidebook` | Patchouli, GuideME, Modonomicon | book reader | to do |

Build and test: `cargo test` in this folder (a workspace of its own, against this checkout's mod API).
