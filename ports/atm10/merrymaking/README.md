# Mama's MerryMaking (Horizon port, partial)

Port of [Mama's MerryMaking](https://www.curseforge.com/minecraft/mc-mods/maidens-merry-making)
1.21.1-17 (ATM10, MIT, by Mama_Michelle) as a native Horizon mod. Written from the mod's public
description only. That description (CurseForge, Modrinth) names its content in general terms:
lights and garlands, "magically changing snow-topped stairs/slabs", fences, doors, mantels,
decoratable Christmas trees, "special interactable candle holders for Hanukkah and Kwanzaa",
ugly sweater armour, foods and drinks, mobs, and a guide book. It does not say how any of them
behave, so only the one interaction a public page states is ported, with an assumed rule.

## What it does

| Feature | Behaviour | Status |
|---|---|---|
| `merrymaking:menorah` (9 candles) and `merrymaking:kinara` (7 candles) | A `candles` state property (0 = none lit); light 3 per lit candle, at most 15 | ported, ids and light assumed |
| Light a candle | Flint and steel on a holder lights one more candle (one per click, as one more is lit each night) and wears by one | ported, rule assumed |
| Put out | An empty-handed click puts every candle out, as vanilla candles go out | ported, rule assumed |
| Decorative blocks (lights, garlands, wreaths, fences, doors, mantels, trees, snowy stairs/slabs) | Mostly vanilla block classes: template declarations | not yet: their ids and facts come from `horizon-port-pack carry` on the jar |
| Decorating a Christmas tree, the "changing" snowy stairs/slabs | Behaviour not described publicly | not yet |
| Ugly sweater armour, foods and drinks | Item declarations with components | not yet: from the jar |
| Mobs (GeckoLib models) | | not yet |
| Guide book (Patchouli) | | not yet |
| Achievements (advancements) | data | from the jar |

## Needs from Horizon

- Mobs: GeckoLib-animated entities need a GeckoLib model/animation loader on the client
  (`.geo.json` + `.animation.json`), e.g. a `register_entity_renderers` renderer kind
  `geckolib` reading `assets/<ns>/geo/` and `assets/<ns>/animations/`. Not found in the API.
- Guide book: a Patchouli book reader (the mod depends on Patchouli). Not found in the API.
- Dependency: the 1.21.1 build also requires Mama's Herbs and Harvest, which is not ported here.

## To check against the NeoForge original (oracle on a machine with the jar)

1. The holders' ids, their state properties, and how they are lit: flint and steel, a candle,
   an empty hand? One candle per click, or one per in-game day? The shamash first?
2. Their light level per candle and their shape.
3. Every other block, item and mob: list them from the jar's data and carry them as template
   declarations; then decide which need code.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
