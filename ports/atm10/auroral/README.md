# Auroral (Horizon port)

Port of Auroral 1.21.1-1.1.3 (ATM10 8.x, MIT) as a native Horizon mod. Written from the mod's
observed behaviour and public description, not from its code. What the original adds beyond
auroras in the sky, and every rule below, is from memory and must be checked against the
original. The original author's name could not be confirmed while writing this port (no
network access): fill it in `horizon-mod.toml` and `LICENSE-ORIGINAL.md` from the mod page.

## What it does

| Feature | Status |
|---|---|
| Auroras in the night sky: rippling green and violet curtains high in the north | ported (client), not yet checked against the original |
| Only at night, fading in after dusk (13000) and out before dawn (23000) | ported |
| Only over cold biomes (full at temperature 0 and below, none from 0.3), weaker in rain | ported |
| Overworld only | ported |
| Colours, height, speed and biome rules from a config | not yet: fixed to the values in `src/lib.rs` |
| Anything the original adds besides the sky effect (it is listed as needed on servers too: blocks, items or a sync of the weather?) | not yet: unknown, to check |

How: `Hooks::render_level_stage` at `RenderLevelStage::AfterSky` reads the camera's level,
biome (`client_biome`, `biome_properties`), weather and the client clock, computes `strength`
and submits `curtains` as `WorldQuad`s (alpha-blended, depth tested, so terrain covers them).
A view whose quad budget is full skips the aurora for that frame.

## To check against the NeoForge original (oracle on a machine with the jar)

1. When auroras show: the hours, the fade, and which biomes (temperature, a biome tag, or
   snowy biomes only).
2. Whether rain and thunder hide them, and whether the moon phase matters.
3. Their colours, height and distance, and how they move.
4. Whether they show in other dimensions.
5. What the server half of the mod does.

## Needs from Horizon

- A quad batch with per-corner colours for world geometry, so a curtain fades from green at the
  bottom to violet at the top in one quad: `WorldQuad::gradient(corners: [[f64; 3]; 4],
  colors: [u32; 4], depth: WorldDepth) -> WorldQuad` in
  `crates/horizon-mod-vocab/src/client/world_draw.rs`. Today each band is one flat colour.
- Nothing else: an additive glow is possible with a mod render pipeline (`pipeline_quads`),
  which this port does not use yet.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
