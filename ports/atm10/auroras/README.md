# Auroras (Horizon port)

Port of Auroras 1.6.2 by Verph (ATM10 8.x, BSD-2-Clause) as a native Horizon client mod.
Written from the mod's public description (its Modrinth and CurseForge pages: auroras fixed
above the clouds in cold biomes, from sunset to sunrise, not in rain or thunder, blue, green and
aqua with a red tint as the night goes on; the 1.5.3 changelog's polar points and band
distance), not from its code. Nobody has compared it with the original in game yet.

## What it does

| Feature | Status |
|---|---|
| Curtains of light from y 200 to 260, east-west bands every 96 blocks, swaying slowly, drawn after the sky so terrain and clouds cover them | ported, shapes are this port's own |
| Only from sunset to sunrise, fading in and out over dusk and dawn; hidden by rain and thunder | ported |
| Snowy biomes (temperature below 0.15) every night; cold biomes (below 0.3) about one night in four, the same nights for every player | ported; the original's "rarely in colder ones" rule is not public |
| Blue, green and aqua along each band, the tops turning red toward sunrise | ported, colours approximate |
| Smooth fade (2.5 s) when walking into or out of a cold biome | ported |
| Magnetic poles: auroras gathered around polar points, rotating when crossing them, pole offset changing day to day (1.5.3) | not yet: how the poles are placed isn't public |
| Config (band distance, rotation speed, polar offset, colours, heights) | not yet: fixed to the constants in `src/lib.rs` |
| Simple Clouds and Aether II compatibility | not applicable |

The mod has no user-facing text, so it ships no lang file.

## Needs from Horizon

Nothing for what is ported. The curtains are plain `WorldQuad`s at `RenderLevelStage::AfterSky`;
their far edge (about 280 blocks away) can be cut by the far plane at the lowest render
distances. A sky-space draw (geometry relative to the camera, not depth tested against the far
plane, as vanilla draws the sun) would be the right home; proposal if it is wanted:
`WorldDrawContext::sky_quads(&mut self, quads: &[SkyQuad]) -> Result<(), DrawError>` in
`crates/horizon-mod-vocab/src/host/draw.rs`, with `SkyQuad { corners: [[f32; 3]; 4] /* camera-relative */, color: u32 }`,
drawn only at `AfterSky`.

## To check against the original (on a machine with the jar)

1. Height, length, spacing and shape of the curtains; whether they are fixed in the world or
   follow the camera.
2. Exactly which biomes show them and how often a merely cold biome does.
3. The hours (does it start at sunset 12,000 or at full night?), the fade, and the red tint's
   timing and colour.
4. How the 1.5.3 polar points work, which this port leaves out.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
