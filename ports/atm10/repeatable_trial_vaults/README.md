# Repeatable Trial Vaults (Horizon port)

Port of [Repeatable Trial Vaults](https://www.curseforge.com/minecraft/mc-mods/repeatable-trial-vaults)
1.0.3 (ATM10 8.x, MIT, by Buuz135) as a native Horizon mod. Written from the mod's public
description ("reset trial vaults by consuming another key"), not from its code.

## What it does

| Behaviour | Status |
|---|---|
| A player the vault already rewarded uses an item of `#repeatable_trial_vaults:can_reset_trial_vaults` (default: `minecraft:trial_key`, `minecraft:ominous_trial_key`) on an idle (`active`/`inactive`) vault: the vault forgets them (`server_data.rewarded_players`) and turns `active` | ported, not yet checked against the original |
| When that item is the vault's own key (`config.key_item`, so a trial key on a normal vault, an ominous key on an ominous one), vanilla then takes it and unlocks: one key, one more reward | ported |
| Another item of the tag (a pack may add one) is consumed for the reset alone; the player then unlocks with a key as usual | ported |
| A player the vault never rewarded, or a vault busy unlocking/ejecting, is left to vanilla | ported |
| Client shows which items can reset vaults (tooltip) | not yet: the port is server-only |

How: `use_item_on_block` (block phase, server) runs before the vault's own use. It reads the
vault's saved data (`BlockEntityRef::data`), removes the player's UUID, writes it back with
`replace_data`, and sets the block's `vault_state` to `active`, since vanilla only takes a key on
an active vault.

## Needs from Horizon

- Nothing for the core. Clearer than editing the saved NBT would be typed vault access:
  `Host::vault_rewarded_players(level: LevelRef, pos: [i32; 3]) -> Option<Vec<u128>>` and
  `ServerHost::vault_forget_player(level: LevelRef, pos: [i32; 3], player: u128) -> bool`.

## To check against the NeoForge original (oracle on a machine with the jar)

1. Whether the reset and the unlock take one key or two (here: one, when the item is the
   vault's key).
2. Whether a key of the wrong kind (a trial key on an ominous vault) may reset it (here: yes,
   it is consumed for the reset, as the tag allows it).
3. Whether the vault forgets only the clicking player or every rewarded player.
4. Sounds or particles on reset (none here beyond vanilla's unlock).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
