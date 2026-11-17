# AE2WTLib (Horizon port: planned)

What it does in ATM10 (from its public description): adds more wireless terminals to AE2 (wireless crafting,
pattern access and pattern encoding terminals) and the **Wireless Universal Terminal**, which holds several of
them in one item and switches between them; terminals draw from a rechargeable battery (an AE energy store in
the item) and can be boosted past an access point's range with infinity booster cards (no limit, even across dimensions).

Where the [AE2 port](../ae2/) stands for it:

- **Already there:** `ae2:wireless_terminal` (link to an access point by sneak-using it, range 16 blocks, opens the
  network's terminal view); the terminal view with search, take out, put in and crafting requests.
- **Next, in this order:** a battery component on the terminal item (energy in `minecraft:custom_data`, charged in
  an energy cell), range cards (`ae2wtlib:infinity_booster_card` as an upgrade), the crafting terminal view (a
  3x3 grid beside the network list; a mod menu with 2x3 and 3x3 grids is needed from the engine), the pattern
  encoding terminal (needs the AE2 pattern terminal first), and the universal terminal (one item, mode switch).

Not ported yet: the engine's mod menu has no crafting-grid-plus-list layout, and the AE2 port has no pattern
terminal. Licence of the original: MIT (docs/modpacks/atm10-mods.csv), so a committed port is allowed.
