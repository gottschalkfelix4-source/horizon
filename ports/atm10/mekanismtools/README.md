# Mekanism Tools (Horizon port)

Port of Mekanism Tools 10.7 (ATM10 8.x, MIT) as a Horizon mod beside the [Mekanism port](../mekanism/).
Written from observed behaviour, not from the mod's code. It is data: `gen_data.py` writes one item
file per tool and armor piece, `src/lib.rs` has no behaviour.

| Part | Status |
|---|---|
| Sword, pickaxe, axe, shovel, hoe and four armor pieces in bronze, lapis lazuli, osmium, refined glowstone, refined obsidian and steel (54 items) | ported: durability, damage, armor, toughness, knockback resistance; looks and mining speed/tier are iron's |
| Paxels, shields, mining speed and tool tier per material, repair materials, enchantability | not yet |
| Armor effects (refined glowstone and obsidian bonuses), Meka-Tool and MekaSuit | not yet (Mekanism's own, with the QIO and gas machines) |
| Crafting recipes | not yet (`horizon-port-pack carry`) |

## To check against the NeoForge original

1. Every number in `gen_data.py` (all from memory).
2. Whether Mekanism's armor sets have the same four durability multipliers as vanilla.

## Build

```sh
sh ../dev-sdk.sh && cargo test && python3 gen_data.py
```

## Paxels

Pickaxe, axe and shovel in one, in the six materials (`gen_data.py`): mining speed 5 to 9 by material and twice the tool durability, both from memory; to check against the original.
