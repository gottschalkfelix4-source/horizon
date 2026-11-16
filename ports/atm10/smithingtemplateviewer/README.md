# Smithing Template Viewer (Horizon port)

Port of Smithing Template Viewer 1.0.4 (ATM10 8.x, MIT) as a native Horizon client mod. Written
from the mod's observed behaviour and public description, not from its code.

## What it does

| Feature | Status |
|---|---|
| Hovering an armour trim smithing template shows a preview in its tooltip: an armour stand in iron armour with the template's trim | ported, not yet checked against the original |
| The trim material cycles once a second through vanilla's materials (quartz, iron, netherite, redstone, copper, gold, emerald, diamond, lapis, amethyst, resin), its name under the stand | ported, not yet checked |
| The stand turns slowly so the back of the trim shows | ported (Horizon's choice; check the original) |
| The template's pattern comes from the client's smithing trim recipes, else vanilla's `<pattern>_armor_trim_smithing_template` naming | ported |
| Vanilla's 18 trim templates | ported |
| Other mods' trim templates | needs API (below) |
| Choosing the armour (material, pieces) or the material shown, config | not yet |

The template's own tooltip text (vanilla's "Smithing Template", pattern name, "Applies to") is
left as vanilla writes it. Names shown are vanilla's translation keys
(`trim_material.minecraft.<material>`), so the port carries no strings of its own.

## Needs from Horizon

- **A tooltip image for items the mod doesn't know at load.** `tooltip_component` and
  `tooltip_component_draw` are item-scoped (`ItemPart`), so a part can only be attached to items
  named in `register`; another mod's templates (ATM10 has several) are unknown there. Proposed: an
  unscoped client hook in `crates/horizon-mod-vocab/src/hooks/tooltip_component.rs`,
  `fn tooltip_component_any(stack) -> Option<[f32; 2]>` (contract `replace`, out `size`, the first
  mod answering wins) with its `tooltip_component_any_draw` counterpart, or
  `Registrar::item_part_for_tag(tag: &str, part)` attaching a part to every member of an item tag
  (`minecraft:trim_templates`) when tags load.

## To check against the original (on a machine with the jar)

1. Whether the preview needs Shift held, and whether it shows in the tooltip or as a separate
   screen/overlay.
2. Which armour the stand wears (iron assumed), whether it turns, and the cycle's order and speed.
3. Whether the material's name is shown, and in which colour.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
