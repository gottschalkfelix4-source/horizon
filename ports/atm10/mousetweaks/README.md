# Mouse Tweaks (built into Horizon)

[Mouse Tweaks](https://github.com/YaLTeR/MouseTweaks) 2.26.1 (ATM10 8.6, BSD-3-Clause, by YaLTeR) needs no
port: Horizon has its controls built in, as the **Mouse Tweaks** and **Mouse Tweaks wheel** extras
(Options > Extras; `mouseTweaks` and `mouseTweaksWheel` in `horizon-extras.txt`). Code:
`crates/horizon-client/src/game/interface/extras/mouse_tweaks.rs`; settings:
`crates/horizon-client-render/src/extras.rs` (`mouse_tweaks`, `mouse_tweaks_wheel`). This folder holds no
crate.

A modpack that ships Mouse Tweaks wants it on: turn on the **Mouse Tweaks** extra (off by default in
Horizon, so vanilla's clicks stay vanilla's for players who didn't ask). The wheel extra is on by default
and acts only while Mouse Tweaks is on.

## Features

| Mouse Tweaks feature (its config option) | Horizon | Status |
|---|---|---|
| RMB tweak (`RMBTweak`): right-drag with a stack puts one item in each slot entered | `mouse_tweaks`: right-drag, one item into each slot entered, the first at the press | built in |
| LMB tweak with item (`LMBTweakWithItem`): left-drag with a stack gathers the same items into the hand; with Shift, quick-moves them | `mouse_tweaks`: the same, while vanilla's own drag hasn't spread over a second slot | built in |
| LMB tweak without item (`LMBTweakWithoutItem`): Shift + left-drag with an empty hand quick-moves every slot entered | `mouse_tweaks` | built in |
| Wheel tweak (`WheelTweak`): the wheel over a slot moves its items one at a time between the two sides | `mouse_tweaks_wheel`: down sends one where Shift-click would (fuel to the fuel slot), up pulls one of the same item from the other side; a result slot gives one craft / the furnace output | built in |
| Wheel search order (`WheelSearchOrder`: first to last / last to first) | fixed: the other side's hotbar first, from the right | gap |
| Wheel scroll direction (`WheelScrollDirection`: normal, inverted, inventory-position aware) | fixed: down pushes, up pulls | gap |
| Scroll item scaling (`ScrollItemScaling`: proportional / always one per notch on high-resolution wheels) | wheel steps are summed and taken whole, so a trackpad's fractions add up (proportional) | partly |
| Each tweak on or off on its own | one switch for the three drags, one for the wheel | gap |
| Every container, mods' included (its API for modded screens) | the inventory, chest-like containers (chests, barrels, ender chests, shulker boxes, hoppers, dispensers/droppers), furnaces, blast furnaces, smokers, crafting tables; not anvils, enchanting, merchants, beacons, crafters | gap for mod and other vanilla menus |
| Works on servers | every action is made of ordinary slot clicks, so it works wherever vanilla's clicks do | built in |

## Gaps, and where they would go

All in Horizon's own extras (the engine, not a mod crate), so none needs a port here:

1. **Separate switches and the two wheel options**: `rmb_tweak`, `lmb_tweak_with_item`,
   `lmb_tweak_without_item`, `wheel_search_order`, `wheel_scroll_direction` fields beside `mouse_tweaks` in
   `crates/horizon-client-render/src/extras.rs`, read in `mouse_tweaks.rs` (`tweak_press`,
   `tweak_drag_move`, `other_side`, `tweak_scroll`).
2. **More menus**: `tweak_menu` lists the vanilla menus by name. Mod menus whose slots behave as storage
   would need a way for a mod to say so (as Mouse Tweaks' own API lets a mod mark its screen), e.g. a
   `MenuPart` flag `mouse_tweaks: bool` in `crates/horizon-mod-vocab`; the stonecutter, loom, cartography
   table, smithing table and grindstone could be added to `tweak_menu` after checking their result slots.

## To check against the original (on a machine with the jar)

1. The wheel's search order and which side a pull takes from in each container.
2. Whether a right-drag skips slots that already hold a different item, and stops when the hand runs out.
3. Which vanilla screens Mouse Tweaks acts in (its handlers for the creative inventory, villager trades).
