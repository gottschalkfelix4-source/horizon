# Silent Gear (Horizon port)

Port of [Silent Gear](https://github.com/SilentChaos512/Silent-Gear) 4.2.1.1 (ATM10 8.x, MIT) as a
native Horizon mod. Written from the mod's observed behaviour and public description (its wiki,
as remembered), not from its code or data. First slice: gear stats computed from materials, and
gear that never breaks.

## What it does

| Feature | Status |
|---|---|
| Gear stats from the main part's materials: durability, mining speed and attack damage averaged, harvest tier the highest, times a synergy for mixed materials | ported, numbers and synergy rule from memory |
| Gear items `silentgear:pickaxe` and `silentgear:sword` with their materials recorded in `minecraft:custom_data` → `silentgear` | partly: two of the original's many gear types |
| Attack damage and speed modifiers from the stats (`item_attribute_modifier`) | ported |
| Durability (`minecraft:max_damage`) and the pickaxe's mining speed and harvest tier (`minecraft:tool`) written from the stats while a player carries it | ported |
| Gear never breaks: wear stops one point short; broken gear hits like a fist | ported; broken gear still mines at full speed (not yet) |
| Materials: wood, stone, flint, iron, gold, emerald, diamond, netherite | partly: the original has dozens, data-driven, plus mod materials |
| Crafting gear from parts and parts from materials (blueprints, templates, the crafting grid) | not yet: see Needs from Horizon. A gear item is made with `/give` for now (below) |
| Rods, tips, coatings, grips, bindings and their stat modifiers; traits (Malleable, Magnetic, Soft, ...) | not yet |
| Repair with materials and repair kits; salvager; gear levelling and the Starlight Charger | not yet |
| Tooltips with stats, part models and coloured textures | not yet |

```
/give @s silentgear:pickaxe[custom_data={silentgear:{materials:["iron","iron","diamond"]}}]
```

## To check against the NeoForge original

1. Each material's stats (the port uses vanilla's tool values where vanilla has the material).
2. How stats of several main materials combine (the port: plain average; tier: the highest) and
   the synergy formula (the port's own: +10 % per further distinct material, -0.5 % per rarity
   point of spread, kept in 50-150 %). This is the part least likely to match.
3. Attack damage and speed per gear type (the port: vanilla's pickaxe and sword values).
4. What a broken tool can still do (mine slowly? not at all?).
5. The stored form: the original uses its own data components; the port uses
   `minecraft:custom_data`.

## Needs from Horizon

- A crafting rule computed from the grid, with vanilla's consumption. `prepare_item_craft` only
  runs when no recipe matched, does not hand over the grid, and taking a result no recipe made
  gives the ingredients back (vanilla's `ResultSlot` copies them when no recipe is found), so
  building gear there would duplicate items. Proposed, in
  `crates/horizon-mod-vocab/src/item/crafting.rs`:
  `trait CraftingRule: Send + Sync { fn assemble(&self, host: &dyn Host, grid: &CraftingGrid) -> Option<ItemStack>; fn remaining(&self, grid: &CraftingGrid) -> Vec<Option<ItemStack>> { /* vanilla default */ } }`
  with `CraftingGrid { width: u8, height: u8, items: Vec<Option<ItemStack>> }`, registered by
  `Registrar::crafting_rule(&mut self, id: &str, rule: impl CraftingRule)` and matched like a
  special recipe (`CustomRecipe`), consuming one of each used slot.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
