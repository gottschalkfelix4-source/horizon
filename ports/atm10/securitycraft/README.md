# SecurityCraft (Horizon port)

Port of [SecurityCraft](https://github.com/Geforce132/SecurityCraft) 1.10.2.1 (ATM10 8.x, MIT) as a
native Horizon mod. Written from the mod's observed behaviour and public description (its wiki and
in-game manual, as remembered), not from its code. First slice: reinforced blocks and ownership.

## What it does

| Feature | Status |
|---|---|
| Ownership: a reinforced block placed from its item belongs to its placer (chunk attachment `securitycraft:owners`, saved with the chunk) | ported, not yet checked against the original |
| Only the owner breaks an owned reinforced block; anyone else's break is cancelled and they are told who owns it (`securitycraft.message.owned_by`) | ported, not yet checked |
| Reinforced blocks are explosion-proof | partly: bedrock's resistance as a block fact in `data/` |
| Reinforced variants of vanilla blocks | partly: four of them (`reinforced_stone`, `reinforced_cobblestone`, `reinforced_oak_planks`, `reinforced_glass`); the original has hundreds |
| Reinforced blocks cannot be pushed by pistons | not yet (`BlockPart::piston_push_reaction` exists for it) |
| Non-owner sees no break progress (client-side break speed 0) | not yet: the client predicts cracks and the server puts the block back |
| Universal Block Reinforcer (convert vanilla blocks), Universal Block Remover / Modifier | not yet |
| Keypads, passcode-protected chests and doors, keycards and keycard readers | not yet |
| Mines, cameras and the security camera monitor, retinal / inventory scanners, lasers, alarms, sentries, Block Change Detector, Sonic Security System | not yet |
| Modules (allowlist / denylist, disguise, ...) and the owner's Briefcase | not yet |
| Config (who may break, ops' rights) | not yet: fixed to the behaviour above |

A block placed without a player's item (dispenser, `/setblock`, a structure) has no owner and
anyone can break it. When the owner breaks the block the record goes with it.

## To check against the NeoForge original

Everything here is from memory and needs an oracle on a machine with the jar:

1. Whether owners break their reinforced blocks at the vanilla counterpart's speed, and whether
   creative players or operators bypass ownership.
2. Which reinforced blocks are explosion-proof (all of them, or not glass), and their exact
   hardness: the `data/securitycraft/horizon/block/*.json` files are placeholders for what
   `horizon-port-pack carry` reads from the jar.
3. The owner message's wording and translation key (the port uses its own key
   `securitycraft.message.owned_by`, in `assets/securitycraft/lang/en_us.json`).
4. Whether ownership is stored on the block entity (as the original does, with owner name and
   UUID) and copied by pick-block; the port keeps it on the chunk instead.

## Needs from Horizon

None for this slice. Showing a non-owner no cracks at all would want the owner records on the
client: a synced attachment (`AttachmentType { sync: true }`) read in `player_break_speed`, which
the API already has; it is listed as "not yet" above, not as a gap.

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
