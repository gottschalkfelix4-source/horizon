# ATM10 libraries on Horizon

Libraries that ATM10's mods share, rebuilt as crates on Horizon's mod API. A ported mod depends on
the one it needs by path (`../libs/<name>`). They are written from public documentation and
observed behaviour, never from the libraries' code.

| Crate | Replaces | What it gives | Status |
|---|---|---|---|
| `config` | NeoForge `ModConfigSpec`, Configuration, Cloth Config | typed schema, TOML file with comments and ranges, correction on load | done, unit-tested |
| `geo` | GeckoLib | `.geo.json` models, `.animation.json` with Molang and easings, entity renderer with a controller | done, unit-tested; not yet seen in the game |
| `accessories` | Curios | slot types and entity rules from data, per-player worn items as a saved attachment, equip/unequip with `curios:<slot>` tag check | data and storage done, unit-tested; screen, tick/equip callbacks and death drops to do |
| `guidebook` | Patchouli, GuideME, Modonomicon | Patchouli books (categories, entries, typed pages), text markup and pagination | Patchouli data side done, unit-tested; GuideME and Modonomicon readers and the screen to do |

Build and test: `cargo test` in this folder (a workspace of its own, against this checkout's mod API).

## Open API gaps found

- Attachment codecs run without a host, so an `ItemStack` can't be encoded in one: `accessories`
  keeps stacks as item NBT and converts at the call boundary. A host-aware codec would simplify it.
- No screen API for a mod-owned container with arbitrary slot layout yet checked (Curios' screen)
  and no text-wrapping measure outside a draw context (`guidebook::markup::paginate` takes one).
