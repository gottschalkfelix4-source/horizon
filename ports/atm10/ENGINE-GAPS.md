# Engine gaps found by the ATM10 ports
Collected from each port's README, section "Needs from Horizon". Each entry is an API Horizon's mod interface lacks today;
the ports work around it or leave the feature out. An engine thread implements them as separate `Mod API:` commits.

## apotheosis

- Changing a mob's equipment as it spawns, to give the affix item to the mob as the original
  does: `finalize_spawn` gets `&dyn Host` (read-only). Proposed: pass `&mut dyn ServerHost` in
  `Hooks::finalize_spawn` (`crates/horizon-mod-vocab/src/hooks/finalize_spawn.rs`), so
  `LivingRef::set_equipment` can be called on the new mob.
- A guaranteed drop for that slot: `LivingRef::set_drop_chance(self, host: &mut dyn ServerHost, slot: EquipmentSlot, chance: f32) -> bool`
  (vanilla's `Mob.setDropChance`), in `crates/horizon-mod-vocab/src/host/living.rs`.

## appleskin

- **Exhaustion on a remote client.** Vanilla sends food and saturation to the client
  (`ClientboundSetHealthPacket`) but not exhaustion; AppleSkin's server half sends it with its
  own payload. The port reads `PlayerRef::food(host).exhaustion` and so shows whatever the client
  holds. Either a server half here (side = "both", a payload sent from `player_tick_post` when
  the value changes, read in the client) or an engine-side `PlayerFood::exhaustion` kept in sync
  on the client would do; the first needs nothing from the engine and is the next step.
- **Riding state on the client**, so the overlay skips the food row when a mount's hearts
  replace it: `PlayerRef::vehicle_max_hearts(self, host: &dyn ClientHost) -> Option<i32>`
  (`crates/horizon-mod-vocab/src/host/players.rs`, client area), or the `HealthInputs` the
  `health_hud` hook already carries passed to `gui_layer_pre`/`gui_layer_post` for
  `minecraft:player_health` (`GuiLayerPostEvent::health: Option<&HealthInputs>`).

## artifacts

- Accessory slots, as Curios or Accessories give: a player inventory section a mod declares and reads.
  Proposed: `fn declare_accessory_slot(&mut self, id: &str, size: u32) -> Result<(), String>` on the
  `Registrar` (`crates/horizon-mod-vocab/src/parts/`), `fn player_accessories(&self, player: PlayerRef,
  slot: &str) -> Option<Vec<ItemStack>>` on `PlayersBackend` (`crates/horizon-mod-vocab/src/host/players.rs`),
  and `inventory_tick` firing for those slots with their own slot numbers. Worked around with vanilla
  equipment slots.
- Reading a player's worn items inside a combat hook: `fn living_equipment(&self, entity: i32, slot:
  EquipmentSlot) -> Option<ItemStack>` on `LevelRandomHost` (`crates/horizon-mod-vocab/src/world/types.rs`).
  Worked around with a set of wearers updated each tick.

## auroral

- A quad batch with per-corner colours for world geometry, so a curtain fades from green at the
  bottom to violet at the top in one quad: `WorldQuad::gradient(corners: [[f64; 3]; 4],
  colors: [u32; 4], depth: WorldDepth) -> WorldQuad` in
  `crates/horizon-mod-vocab/src/client/world_draw.rs`. Today each band is one flat colour.
- Nothing else: an additive glow is possible with a mod render pipeline (`pipeline_quads`),
  which this port does not use yet.

## auroras

Nothing for what is ported. The curtains are plain `WorldQuad`s at `RenderLevelStage::AfterSky`;
their far edge (about 280 blocks away) can be cut by the far plane at the lowest render
distances. A sky-space draw (geometry relative to the camera, not depth tested against the far
plane, as vanilla draws the sun) would be the right home; proposal if it is wanted:
`WorldDrawContext::sky_quads(&mut self, quads: &[SkyQuad]) -> Result<(), DrawError>` in
`crates/horizon-mod-vocab/src/host/draw.rs`, with `SkyQuad { corners: [[f32; 3]; 4] /* camera-relative */, color: u32 }`,
drawn only at `AfterSky`.

## betteradvancedtooltips

- **An item's tags.** `Tags` (`crates/horizon-mod-vocab/src/world/tags.rs`) answers whether an
  item is in a tag and a tag's members, not which tags an item has. Proposed:
  `fn item_tags(&self, item: ItemKey) -> Option<Vec<Identifier>>` on `Tags` (the merged item tags
  the client has, sorted by id; None while tags are unavailable).

## colorfulhearts

- **Replace only the hearts.** `health_hud` replaces armour, hearts, food and air together, so
  this port carries a copy of vanilla's other rows, and any other mod that answers
  `health_hud` (or AppleSkin's overlay, which assumes vanilla's stacked rows above 20 health)
  cannot combine with it. Proposed: a hook `player_hearts` in
  `crates/horizon-mod-vocab/src/hooks/player_hearts.rs`,
  `fn player_hearts(inputs: &HealthInputs, x_left: i32, y_base: i32) -> Option<HeartRows>`
  (contract `replace`), where `HeartRows { draws: Vec<GuiDraw>, rows: i32, row_height: i32 }`
  tells the engine how tall the hearts are so it places armour above them itself. Also exposing
  that answer (`rows`, `row_height`) to `gui_layer_post` for `minecraft:player_health` would let
  overlays such as AppleSkin's find the hearts.

## controlling

1. **Every key binding, readable and writable.** In `crates/horizon-mod-vocab/src/host/client_input.rs`
   (`ClientInputClientHost`):
   ```rust
   /// Side: client. Every mapping Controls lists, vanilla's, Horizon's and every mod's, in its order.
   fn key_bindings(&self) -> Vec<KeyBindingView>;
   /// Side: client. Rebinds `id` (`key.jump`, or `<mod id>:<id>`) as Controls does and saves it;
   /// `key.keyboard.unknown` unbinds it.
   fn set_key_binding(&mut self, id: &str, key: &str, modifiers: ClientModifiers) -> Result<(), MappingError>;
   ```
   with `KeyBindingView { id, label: Text, category: String, key: String, modifiers: ClientModifiers,
   default_key: String, default_modifiers: ClientModifiers, context: KeyConflictContext, owner: Option<String> }`
   in `client/key_mapping.rs`. It would list other mods' keys, and turn on rebinding and Reset All.
2. **Know the Key Binds page.** `ScreenView::id` reports `options` for every options page. An id per page
   (`options.key_binds`) would let the search box sit on vanilla's own screen, as Controlling's does, and
   keep the **Search keys** button off the other pages.
3. **Open a mod screen from a screen hook.** The screen hooks hold `&dyn ClientHost`, so the button's click
   is acted on at the next client tick, which closes the options screens before opening the mod's. A
   `ScreenOpening`-style answer from `screen_mouse_button_pressed_pre` (or `mod_screen_open` on `&self`)
   would open it straight over them and return to them on Escape.

## darkmodeeverywhere

**A colour filter for the GUI.** A hook asked once a frame for the open screen, in a new
`crates/horizon-mod-vocab/src/hooks/gui_color_filter.rs`:
```rust
/// The open screen is about to draw (`screen` as the screen hooks see it): set `filter` to recolour
/// its textured GUI quads (sprites, nine-slices, the panel), and `text_filter` its text, until the
/// screen is drawn. Items, entities and the world are never filtered. Client-side.
#[hook(side = "client", contract = "listen", neoforge = "none (Horizon-only)")]
fn gui_color_filter();
// event: GuiColorFilterEvent<'a> { screen: &'a ScreenView<'a>, filter: &'a mut Option<ColorMatrix>,
//                                  text_filter: &'a mut Option<ColorMatrix> }
```
with `ColorMatrix { rows: [[f32; 5]; 4] }` (RGBA out = matrix × RGBA1 in) in `client/types.rs`, applied in
the GUI pipeline's fragment shader. With it the port keeps light-grey `#C6C6C6` → dark `#2B2B2B`,
dark text `#404040` → light, and buttons and panel-less screens darken too, as the original's shader does.

## deeperdarker

- A level's portal shape for mod portals, so a mod portal block collapses when its frame breaks
  and is found from the far side like a nether portal:
  `trait PortalShape { fn find(&self, level: &dyn LevelHost, pos: [i32; 3]) -> Option<Opening>; }`
  with `Registrar::register_portal(block: BlockKey, frame: &str /* block tag */, destination: &str, shape: impl PortalShape)`
  in `crates/horizon-mod-vocab/src/parts/` (beside `dimensions.rs`). Today the port counts ticks
  itself and cannot collapse the portal.
- Cross-level passages: `Passage::destination_level` is refused when crossed (`FarLevel`), so the
  port cannot use passages for travel.

## dummmmmmy

- A mod living entity type that is not a mob template: `GeneralTypesRegistrar::declare_living_entity(
  &mut self, path: &str, category: MobCategory, width: f64, height: f64, attributes: &[(&str, f64)]) ->
  Result<EntityTypeKey, GeneralEntityRegistrationError>` (`crates/horizon-mod-vocab/src/parts/entity_types.rs`),
  or `minecraft:mannequin` / `minecraft:armor_stand` accepted as an `entity_type/*.json` template.
  Worked around with a tagged vanilla mannequin.
- Reading an entity's tags inside a combat hook: `fn entity_tags(&self, entity: i32) -> Option<Vec<String>>`
  on `LevelRandomHost` (`crates/horizon-mod-vocab/src/world/types.rs`). Worked around with a set of known
  dummy ids kept from `entity_join_level` / `entity_leave_level`.
- Floating damage numbers need a client mod and a payload; not attempted in this server-side slice.

## dungeoncrawl

- Nothing for this slice: the structure type, its pieces and block entity NBT are in the API.
  Themes need a biome read in `PieceContext` (`fn biome(&mut self, pos: [i32; 3]) -> BiomeId`,
  beside `GenerationContext::noise_biome` in `crates/horizon-mod-vocab/src/world/structure_type.rs`),
  to pick a theme per piece where the original does.

## enchdesc

- **Enchanting table clues.** The table's hover tooltip (`EnchantmentScreen`, "Enchantment
  level?") is not an item tooltip. Proposed: a client hook `enchantment_clue_tooltip` in
  `crates/horizon-mod-vocab/src/hooks/enchantment_clue_tooltip.rs` with
  `EnchantmentClueTooltipEvent { slot: u8, enchantment: &str, level: u32, lines: &mut Vec<(Text, u32)> }`
  (contract `listen`, NeoForge counterpart: none, a mixin in the original).

## enderio

- None blocking this slice. A capacitor-only slot can already be fenced with `menu_slot_rules`
  (players) and the container's faces (hoppers); the alloy recipes as data could go through
  `recipe_serializers` once ported. Both are left for a later slice.

## eternal_starlight

- Mod portals with the engine's portal handling, so the portal collapses when its frame breaks
  and the far side is found like a nether portal's:
  `Registrar::register_portal(block: BlockKey, destination: &str, shape: impl PortalShape)` with
  `trait PortalShape { fn find(&self, level: &dyn LevelHost, pos: [i32; 3]) -> Option<Vec<[i32; 3]>>; }`
  in `crates/horizon-mod-vocab/src/parts/` (beside `dimensions.rs`).
- Cross-level passages (`Passage::destination_level` is refused as `FarLevel` when crossed).

## evilcraft

- **A block that copies another block's look and drops** (the blood-stained block keeps the
  block it replaced): a content-mod block whose block entity stores an inner `BlockState` and
  whose model and drops forward to it, e.g. `"template": "horizon:mimic"` in
  `data/<id>/horizon/block/*.json` plus `LevelRef::set_mimic(self, host: &mut dyn ServerHost,
  pos: [i32; 3], mimic: BlockKey, inner: BlockState) -> Result<bool, LevelWriteError>` in
  `crates/horizon-mod-vocab/src/host/levels.rs`.

## extremesoundmuffler

1. **Which world the client is in**, to keep anchors per world as the original does: in
   `crates/horizon-mod-vocab/src/host/client_view.rs`, `fn world_key(&self) -> Option<String>`, the save's
   folder name in single player and the server's address in multiplayer (no player name, no IP for LAN).
2. **The client host in `play_sound`.** It gets only a `LoopContext`, so the dimension is copied from the
   last client tick, and the state is shared through a lock that a sound played while the screen holds it
   skips (`try_lock`). A read-only `&dyn ClientHost` there (or `dimension` in `PlaySoundEvent`) would remove
   both.

The `muffle_sounds` hook is not used: it is a short, fading muffle of every sound (ears ringing after a
blast), not a per-sound one.

## framedblocks

- A client model hook keyed by a block's position, so a frame draws its camouflage's model:
  `fn block_model_override(&self, host: &dyn ClientHost, event: &mut BlockModelOverrideEvent) -> Option<BlockState>`
  on `BlockPart` (`crates/horizon-mod-vocab/src/hooks/`), with `event.pos`, `event.state`,
  answering the state whose baked model the mesher draws instead. Today the camouflage is kept on
  the server only; clients draw the bare frame.
- `OnBlockExplodedEvent` with the level it happens in (`level: LevelRef`), so an exploded frame
  can drop its camouflage.

## gateways

- Datapack-defined gateways need nested records: a gateway is a list of waves, each with lists of
  entities and rewards. `Registrar::declare_datapack_registry` takes flat fields only
  (`DatapackFieldKind`: bool, int, float, text, identifier, reference). Proposed: two new variants in
  `crates/horizon-mod-vocab/src/world/datapack_registry.rs`, `DatapackFieldKind::List { item:
  Box<DatapackFieldKind>, max_len: u32 }` and `DatapackFieldKind::Record { fields:
  Vec<DatapackField> }`, with `DatapackValue::List(Vec<DatapackValue>)` and
  `DatapackValue::Record(Vec<(String, DatapackValue)>)`. Worked around with built-in definitions.

The API already has what the next steps need, so they are not gaps: a general entity type with
`EntityPart::general_entity_tick` for the gateway itself (its look needs a client renderer), and a
`Holder::Level` attachment to save open gateways.

## iceandfire

- Mod living entity types with their own classes (dragons): `GeneralTypesRegistrar::declare_living_entity(
  &mut self, path: &str, category: MobCategory, width: f64, height: f64, attributes: &[(&str, f64)]) ->
  Result<EntityTypeKey, GeneralEntityRegistrationError>` with an `EntityPart` for its tick and AI
  (`crates/horizon-mod-vocab/src/parts/entity_types.rs`). Today `entity_type/*.json` copies only zombies,
  villagers, wandering traders, wolves, cats and parrots.

## jei

1. **Recipe pages in the game-drawn panel for native mods.** The game already has
   `native_panel_recipes` (`crates/horizon-client/src/game/native_hooks/shared.rs`) and
   `PanelEvent::{SlotClicked, Action, Page, Back, Tab}` reach native mods, but no
   `RecipesClientHost` method lets a native mod call it. Proposed, in
   `crates/horizon-mod-vocab/src/host/recipes.rs`:
   `fn panel_recipes(&mut self, mod_id: &str, title: &str, tabs: Vec<Stack>, tab: u32, recipes: Vec<PanelRecipe>);`
   with `PanelRecipe` (slots with role and stacks, results, fluid wells, action button) in
   `client/types.rs`, the public form of `NativeRecipeView`. Until then this port draws its
   recipe screen itself over the container (`screen_render_post`).
2. **An item's tags.** `$` can only find tags some recipe names (read from the recipe files),
   because nothing lists the tags an item is in. Proposed in `crates/horizon-mod-vocab/src/world/tags.rs`:
   `fn item_tags(&self, item: ItemKey) -> Option<Vec<Identifier>>` on `Tags`.
3. **Creative tabs and mod names.** For `%` and for `@` on a mod's display name: proposed
   `fn creative_tabs_of(&self, item: ItemKey) -> Vec<Text>` on `ItemsClientHost`
   (`host/items.rs`) and `fn mod_display_name(&self, namespace: &str) -> Option<String>` on
   `ConfigHost` (`host/config.rs`).

## justenoughbreeding

- **What an entity type is bred with.** Breeding items live in each animal's Java `isFood`, which
  vanilla backs with the food tags above but mods need not. Proposed in
  `crates/horizon-mod-vocab/src/host/entities.rs`:
  `fn entity_type_breeding_items(&self, kind: EntityTypeKey) -> Option<Vec<ItemKey>>` (None for a
  type that doesn't breed), and `fn entity_type_spawn_egg(&self, kind: EntityTypeKey) -> Option<ItemKey>`
  for the offspring slot.

## justenoughprofessions

- **A profession's job site blocks.** Proposed in `crates/horizon-mod-vocab/src/host/merchants.rs`:
  `fn villager_professions(&self) -> Vec<(String, Vec<BlockKey>)>`: every registered profession
  (`minecraft:villager_profession`) with the blocks of its acquirable job site POI
  (`VillagerProfession.acquirableJobSite` → `PoiType.matchingStates`). `registry_entries` already
  names the professions, but nothing gives their blocks.

## keybindbundles

1. **Fire any binding.** `ClientHost::click_mapping` refuses mappings outside the movement, gameplay and
   inventory categories (`MappingError::NotGameplay`) and knows no mod's mapping. Proposed, in
   `crates/horizon-mod-vocab/src/host/client_input.rs`:
   ```rust
   /// Side: client. Presses binding `id` (`key.screenshot`, `key.togglePerspective`, or `<mod id>:<id>`)
   /// once, as a press and release of its key does with no screen open: the game's own key handler for a
   /// vanilla mapping (`handleKeybinds`, the window shortcuts), one click for a mod's.
   fn press_binding(&mut self, id: &str) -> Result<(), MappingError>;
   ```
   with the `key_bindings()` listing proposed in `../controlling/README.md` so the editor lists them all.
2. **Key mappings registered at run time** (`ClientHost::register_runtime_key_mapping(KeyRegistration)`,
   kept by mod id like the others), for bundles the player adds and names.

## keybindspurger

1. **List and rebind every key binding**: the `ClientHost::key_bindings()` and
   `ClientHost::set_key_binding(id, key, modifiers)` proposed in `../controlling/README.md`
   (`crates/horizon-mod-vocab/src/host/client_input.rs`, with `KeyBindingView` in
   `client/key_mapping.rs`). With them the purge unbinds (`key.keyboard.unknown`), reaches other mods'
   bindings, and "purge all modded" is `owner.is_some()`.
2. **Hide a binding from Controls**: `fn set_key_binding_hidden(&mut self, id: &str, hidden: bool)` beside
   them, kept by the game with the binding.
3. As for Controlling: an options page id (`options.key_binds`) for a button on the right page, and a way
   to open a mod screen from a screen hook.

## little_big_redstone

- Read access to attachments where `block_signal` runs. Today `block_signal` gets a read-only
  `&dyn LevelHost`, which has no attachment store, so the chip encodes its outputs in its block
  state; analog outputs that way would need 16^4 states. Proposed, in
  `crates/horizon-mod-vocab/src/world/types.rs`:
  `trait LevelHost { fn attachment_saved(&self, holder: Holder, namespace: &str, name: &str) -> Option<Compound> }`
  (read-only, the saved form), so a part answers a signal from its own record.
- A removal notice with write access for every removal of a block (explosion, piston,
  `/setblock`), not only a player's break (`BlockPart::on_destroyed_by_player` covers that one),
  to forget a chip's record and to put its circuit in the drop:
  `BlockPart::on_removed(&self, level: &mut dyn LevelWriteHost, event: &mut OnRemovedEvent { pos: BlockPos, state: BlockState, new_state: BlockState, moved: bool })`
  (vanilla's `BlockBehaviour.onRemove`). Until then a stale record is harmless: it is replaced
  when a chip is placed there again, and a tick only runs for a chip block.

## minecolonies

- World-wide mod data, for colonies that span dimensions and a one-colony-per-player rule across
  them (the level attachment is per dimension, and which level `Holder::Level` means outside a
  level's own hooks is not stated). Proposed: `Holder::World` in
  `crates/horizon-mod-runtime/src/attachments.rs`, saved in the world's `data/` folder, reached
  from every server hook.
- The player in `EntityPlaceEvent` as a `PlayerRef`, so a refusal can be told to the placer
  without going through `host.players().acting_player()` and comparing entities:
  `pub placer_player: Option<PlayerRef>` on `EntityPlaceEvent` (`crates/horizon-mod-vocab/src/hooks/entity_place.rs`).

## modern_industrialization

- `ModFluidTanks::new`: `ModFluidTanks` is `#[non_exhaustive]` with no constructor, unlike
  `ModContainer` and `ModEnergyBuffer`, so a mod builds it from `Default` field by field. Proposed,
  in `crates/horizon-mod-vocab/src/item/containers.rs`:
  `impl ModFluidTanks { pub fn new(tanks: Vec<i32>, faces: [Vec<usize>; 6], inputs: Vec<usize>, outputs: Vec<usize>) -> Self }`.

## moreoverlays

1. **Light levels on a client.** `LevelRef::brightness(host, LightLayer, pos)`
   (`level_brightness` in `crates/horizon-mod-vocab/src/host/levels.rs`) is `Side: server`:
   the game answers it from `native_level_world`, which refuses a client, so on a window joined
   to its server thread (every single-player world) it returns `None` and the overlay draws
   nothing. Proposal: make it `Side: both` and answer on a client from the client's world
   (`native_level_read_world`, as `level_block_state` does; vanilla `ClientLevel.getBrightness`).
   No signature change.
2. **Spawn rules of a block state.** The client has no read of `BlockState.isValidSpawn` or
   `NaturalSpawner.isValidEmptySpawnBlock`, so the port keeps its own list of blocks that never
   take a spawn and ignores redstone signal sources and blocks that hurt mobs. Horizon already
   exports both as flags (`horizon-client/src/mobs/spawn_rules.rs`). Proposal, in
   `host/block_states.rs` (`BlockStatesHost`, `Side: both`, `#[access(Metadata)]`):
   `fn block_state_spawn_rules(&self, state: BlockState, entity: EntityTypeKey) -> Option<SpawnRules>`
   with `SpawnRules { valid_spawn: bool, valid_empty_spawn: bool }`.

## naturesaura

Nothing this slice could not do with the current API. Left to do: move the generator onto a
declared block entity type (`r.block_entity_type`, `BlockEntityPart::block_entity_tick`, which
the API already has) so its memory lives on the block, and write the client half (the Eye's HUD
bar) as a client part.

## neovitae

None found for this slice. Left to do with the API as it is: declare `neovitae:sacrifice` as a
damage type in `data/neovitae/damage_type/` (no armour, its own death message) once it is
checked that a native mod's data registers it, and move the altar onto a block entity type.

## occultism

- **Listening to items a data pack names**: parts can only be attached at registration, before
  recipes load, so an input that only a data pack's `occultism:spirit_fire` recipe names is not
  heard. Proposed: `Registrar::item_part_dynamic(&mut self, part: Arc<dyn ItemPart>, filter: fn(&dyn
  Host, ItemKey) -> bool)` in `crates/horizon-mod-vocab/src/parts/items.rs`, re-evaluated after
  each data reload.
- **A block part for "entity inside" that can write the entity**: `entity_inside` hands a
  `LevelRandomHost`, which cannot change an item entity's stack; a server variant
  `BlockPart::entity_inside_server(&self, host: &mut dyn ServerHost, event: &mut
  EntityInsideEvent<'_>)` would let the fire own the transformation instead of a part on every
  input item, and make it work for inputs a data pack adds.

## pneumaticcraft

- A quiet write of another block entity's saved data. Pushing air into a neighbour goes through
  `BlockEntityRef::merge_data`, which also sends clients the block's update tag
  (`sendBlockUpdated`); a pressure network changes air every tick, so every tube's update goes out
  every tick. Proposed, in `crates/horizon-mod-vocab/src/host/block_entities.rs`
  (`BlockEntitiesBackend`, server side, transactional like `merge_data`, handle method
  `BlockEntityRef::merge_data_quiet`):
  `fn block_entity_merge_data_quiet(&mut self, entity: BlockEntityRef, data: Compound) -> Result<bool, BlockEntityError>`:
  merge and mark the chunk changed (`setChanged`), without `sendBlockUpdated`.

## polymorph

1. **Refresh a crafting result now.** After a pick arrives the server should prepare the result
   again (vanilla `CraftingMenu.slotsChanged`), but `ItemPlace::MenuSlot` does not reach the
   crafting result and no host call re-runs the preparation. Proposal, in
   `crates/horizon-mod-vocab/src/host/menus.rs` (`MenusServerHost`, `Side: server`):
   `fn refresh_crafting_result(&mut self, player: u128) -> bool` that runs the open crafting
   menu's `slotsChanged` (so `prepare_item_craft` is asked again) and sends the result slot.
2. **Pick a furnace's recipe.** Every furnace hook (`furnace_start_smelt`, `furnace_smelt`,
   `block_cook`) only listens. Proposal: a replace hook in
   `crates/horizon-mod-vocab/src/hooks/furnace_recipe.rs`,
   `fn furnace_recipe(host: &mut dyn ServerHost, event: &mut FurnaceRecipeEvent) -> Option<Identifier>`
   with `FurnaceRecipeEvent { pos: [i32; 3], kind: &str /* minecraft:smelting... */, input: &ItemStack, candidates: &[Identifier] }`,
   asked when `AbstractFurnaceBlockEntity` looks its recipe up; the answer must be one of the
   candidates. The furnace screen's result slot box is already readable (`MenuRef::slot_boxes`).
3. **What the taken result consumes.** When the picked recipe's remainders differ from the
   game's first match (a bucket left behind in one and not the other), the game consumes the
   grid by its own match. Proposal: let `prepare_item_craft` return the recipe id it chose,
   `Option<(ItemStack, Identifier)>`, so the take uses that recipe's remainders.

## refinedstorage

- None blocking this slice. Each capability call walks the network (bounded at 4 096 blocks);
  a cached graph, invalidated from the existing `block_changed` hook, is a later optimisation.

## rftoolsbuilder

- Destroying a block as a transaction step, so a full inventory can roll the dig back instead of
  dropping items (transactions cover block entities and inventories, not block changes today):
  `pub fn destroy_block_in(self, host: &mut dyn ServerHost, tx: &Transaction, pos: [i32; 3],
  tool: Option<&ItemStack>) -> Result<Option<Vec<ItemStack>>, StackWriteError>` on `LevelRef`
  (`crates/horizon-mod-vocab/src/host/levels.rs`, `level_destroy_block`'s area).

Not a gap, a to-do here: the card and cursor could live on the Builder's block entity
(`BlockEntityPartState`) instead of the level attachment, so they move and drop with the block.

## securitycraft

None for this slice. Showing a non-owner no cracks at all would want the owner records on the
client: a synced attachment (`AttachmentType { sync: true }`) read in `player_break_speed`, which
the API already has; it is listed as "not yet" above, not as a gap.

## sfm

- A filter given the host for `transfer_matching`, so a mod can match items by id (which needs
  `ItemKey::id(host)`) without listing a storage itself, as `move_items` in `src/lib.rs` does now:
  `pub fn transfer_matching_with(host: &mut dyn ServerHost, from: &Capability<dyn ItemStorage>,
  to: &Capability<dyn ItemStorage>, filter: impl Fn(&dyn Host, &ItemResource) -> bool, max: u64,
  tx: &Transaction) -> Result<u64, StorageError>` in `crates/horizon-mod-vocab/src/item/storage.rs`.

Not gaps, to-dos here: the manager's program and labels could live on a block entity of its own
(`BlockEntityPartState`) so they drop with the block; the program editor and Label Gun need a
mod menu and an item part, both of which the API has.

## shrink

- An item energy store for the device's FE: present in the API (`item_capability`, energy
  capability); not used in this slice. Nothing missing for what is ported.
- Using an item on another player: `item_interact_mob` skips players. Proposed: fire it for players
  too, or a `fn item_interact_player(&self, host: HostSide, event: &mut ItemInteractPlayerEvent<'_>) ->
  Option<InteractionResult>` on `ItemPart` (`crates/horizon-mod-vocab/src/hooks/`).

## silentgear

- A crafting rule computed from the grid, with vanilla's consumption. `prepare_item_craft` only
  runs when no recipe matched, does not hand over the grid, and taking a result no recipe made
  gives the ingredients back (vanilla's `ResultSlot` copies them when no recipe is found), so
  building gear there would duplicate items. Proposed, in
  `crates/horizon-mod-vocab/src/item/crafting.rs`:
  `trait CraftingRule: Send + Sync { fn assemble(&self, host: &dyn Host, grid: &CraftingGrid) -> Option<ItemStack>; fn remaining(&self, grid: &CraftingGrid) -> Vec<Option<ItemStack>> { /* vanilla default */ } }`
  with `CraftingGrid { width: u8, height: u8, items: Vec<Option<ItemStack>> }`, registered by
  `Registrar::crafting_rule(&mut self, id: &str, rule: impl CraftingRule)` and matched like a
  special recipe (`CustomRecipe`), consuming one of each used slot.

## smithingtemplateviewer

- **A tooltip image for items the mod doesn't know at load.** `tooltip_component` and
  `tooltip_component_draw` are item-scoped (`ItemPart`), so a part can only be attached to items
  named in `register`; another mod's templates (ATM10 has several) are unknown there. Proposed: an
  unscoped client hook in `crates/horizon-mod-vocab/src/hooks/tooltip_component.rs`,
  `fn tooltip_component_any(stack) -> Option<[f32; 2]>` (contract `replace`, out `size`, the first
  mod answering wins) with its `tooltip_component_any_draw` counterpart, or
  `Registrar::item_part_for_tag(tag: &str, part)` attaching a part to every member of an item tag
  (`minecraft:trim_templates`) when tags load.

## theurgy

- **A bulk block read of a box**: the search makes one host call per position (274,625 for the
  T2 cube). `LevelRef::blocks_in_box(self, host: &dyn Host, min: [i32; 3], max: [i32; 3]) ->
  Option<Vec<BlockKey>>` (x, then z, then y order; `None` past a size cap or where unloaded) in
  `crates/horizon-mod-vocab/src/host/levels.rs` would make it one call per chunk section.
  `read_columns` only reads around a heightmap.

## toastcontrol

- **Clearing the toast queue**: `fn clear_toasts(&mut self)` on a client host area
  (`crates/horizon-mod-vocab/src/host/client_view.rs`, `ClientViewClientHost`), as
  `ToastManager.clear`. The port would then register a key (`KeyRegistration::new("clear", "key.keyboard.j")`)
  and call it on `consume_click(MOD_ID, "clear")` from `client_tick_post`.
- **Toast display**: a `toast_render` hook (client, `contract = "listen"`) with
  `ToastRenderEvent { kind: ToastKind, visibility: &mut f32, slide: &mut bool, lifetime_ms: &mut u64 }`
  in `crates/horizon-mod-vocab/src/hooks/toast_render.rs`, or `ToastAddEvent` fields
  `display_ms: &mut u64` and `slide: &mut bool`, so a mod can shorten or stop the animation;
  and a client setting for how many toasts show at once (`ToastManager` visible slots).
- **The toast's id** in `ToastAddEvent` (`id: Option<&str>`, the class/registry name a mod's
  toast has), for block lists of mods' toasts.

## torchmaster

- `Host::entity_type_category(kind)`: the category of a mob type before the mob exists. Added
  in this fork (`crates/horizon-mod-vocab/src/host/entities.rs`,
  `crates/horizon-client/src/game/modding/native_entity_data.rs`); to send upstream as a PR.
