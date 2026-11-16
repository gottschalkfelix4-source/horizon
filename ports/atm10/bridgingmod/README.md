# Bridging Mod (Horizon port)

Port of [Bridging Mod](https://modrinth.com/mod/bridgingmod) 2.6.2+1.21.1 (ATM10, MIT) as a
native Horizon client mod. Written from the mod's observed behaviour and public description, not
from its code.

## What it does

| Feature | Behaviour | Status |
|---|---|---|
| Reach-around target | Standing on a block and looking down past its edge (pitch ≥ 25°, crosshair on nothing), the target is the empty cell beside the block underfoot, in the direction the player faces. | ported (`target`), not yet checked against the original |
| Target outline | The target cell is outlined each frame (vanilla outline colour), so the player sees where the block would go. | ported |
| Placing the held block on the target with use (right-click) | | not yet: needs a client placement call, see below |
| Vertical reach-around (placing below while pillaring down), config screen (min pitch, outline toggle), crosshair indicator | | not yet |

Client-only: nothing on the server changes, as in the original, which also works on vanilla servers.

## Needs from Horizon

Placing the block. Bridging Mod sends a normal use-item-on-block for a hit the crosshair did not
give (the side of the block underfoot). The API reads the crosshair (`ClientHost::crosshair_target`)
and can veto a use (`interaction_key_mapping_triggered`), but cannot send a use on another target.
Either of these would do:

- `ClientHost::use_item_on(&mut self, hand: Hand, pos: [i32; 3], face: Direction, hit: [f64; 3]) -> Result<InteractionResult, UseRefusal>`
  (`MultiPlayerGameMode.useItemOn` with a `BlockHitResult`): the mod would cancel the vanilla use
  in `interaction_key_mapping_triggered` when the crosshair misses and a target exists, then call it
  with `support`, `face` and the face's centre, and swing the hand on success.
- A client hook to replace the tick's pick (`Minecraft.hitResult`) after `GameRenderer.pick`,
  e.g. `fn pick_result(&self, host: &dyn ClientHost, event: &mut PickResultEvent)` with
  `event.target: Option<CrosshairTarget>` writable: the mod would put the reach-around block hit
  there when the ray misses, and vanilla's own use code would place the block (and draw the
  outline itself).

## To check against the NeoForge original

1. The pitch threshold and whether it needs the player to be on the block's edge, sneaking, or
   only holding a block item.
2. Whether the target follows the facing (4 directions) or the look vector, and whether it
   also reaches diagonally and downwards.
3. The outline's look (colour, whether it shows a translucent block preview instead).

## Build

```sh
cargo test                                    # in this folder, against the checkout's mod API
horizon-launcher native-build . --instance <instance> --trust-native-code
```
