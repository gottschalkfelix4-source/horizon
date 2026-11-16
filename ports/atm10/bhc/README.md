# Baubley Heart Canisters (Horizon port)

Port of [Baubley Heart Canisters](https://www.curseforge.com/minecraft/mc-mods/baubley-heart-canisters)
1.21.1-1.4.3 (ATM10, MIT) as a native Horizon mod. Written from the mod's public description and
observed behaviour, not from its code.

## What it does

| Feature | Status |
|---|---|
| Hearts drop when a player kills a mob: red from hostile mobs (5 %), orange from the Wither and Elder Guardian, green from the Ender Dragon, blue from the Warden | ported, sources and chances assumed |
| Heart items, empty canister, four heart canisters, Heart Amulet (from `data/bhc/horizon/item`) | items declared |
| A canister used with the amulet in the other hand goes into it (up to 10 of each colour); a full colour refuses it | ported, as a stand-in for the amulet's GUI |
| Each canister in the amulet adds one heart (2 max health) through the `bhc:heart_canisters` modifier on `minecraft:max_health`; health above a lowered max is cut | ported |
| The amulet's tooltip lists its canisters | ported |
| The amulet worn in a Curios necklace slot | not yet: needs accessory slots (below). Here the first amulet anywhere in the inventory counts |
| The amulet's own GUI (4 canister slots, taking canisters back out) | not yet: canisters go in by use and do not come out |
| Recipes (canister, heart canisters, amulet) | carried from the jar (`horizon-port-pack carry`), not written here |
| Soul Heart Amulet, Relic Apple, Blade of Vitality, config | not yet |

## Needs from Horizon

- **Accessory slots.** The original depends on Curios. Horizon's API has no accessory slot system:
  `ItemPlace::Mod` with `Registrar::place_provider` lets a mod expose a place it keeps, but there
  is no slot in the player's inventory screen, no saved per-player accessory inventory and no
  equip/unequip hooks for one. What this port would use:
  - `Registrar::accessory_slot(id: &str, label_key: &str, accepts: TagKey)` declaring a slot type
    (`curios:necklace`), shown on the inventory screen and saved with the player;
  - `PlayerRef::accessories(self, host: &dyn Host) -> Vec<(AccessorySlotId, ItemStack)>`;
  - `ItemPart::on_accessory_equip / on_accessory_unequip(&self, host: &mut dyn ServerHost,
    event: &mut AccessoryChangeEvent { player: PlayerRef, slot: AccessorySlotId, stack: &ItemStack })`.
- **The amulet's GUI** is possible with a mod menu (`parts/menus.rs`) whose slots back onto the
  stack's component; not done in this first pass.

## To check against the NeoForge original (oracle on a machine with the jar)

1. Which mobs drop which heart, and the chances (here red 5 % from any `MobCategory::Monster`,
   bosses 100 %). Whether looting, a player kill or `doMobLoot` matter.
2. Canisters per colour (10 here) and health per canister (one heart here); whether colours must
   be filled in order (red before orange...).
3. Whether the modifier is `add_value` and survives death with keepInventory; whether health is
   cut when the amulet comes off.
4. Item ids and lang keys (`assets/bhc/lang/en_us.json` are this port's).
   `data/bhc/horizon/item/*.json` are placeholders for building without the jar.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
