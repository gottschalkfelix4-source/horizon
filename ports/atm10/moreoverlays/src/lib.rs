//! More Overlays for Horizon: a light level overlay (F7) that marks where monsters can spawn, a
//! chunk boundary overlay (F9) and an inventory search (Ctrl+F) that darkens slots whose item
//! doesn't match.
//!
//! Written from More Overlays Updated's visible behaviour and public description, never from
//! its code (docs/modpacks.md, "Licences"). What still has to be checked against the original,
//! and what Horizon's mod API is missing, is in README.md.
//!
//! The rules are plain functions in `light`, `chunks` and `search`, tested on their own; this
//! file reads the client world and the open screen through the host and draws.
use std::sync::Mutex;

use mod_api::client::widgets::{TextField, WidgetId, Widgets};
use mod_api::{
    ClientHost, ClientLevelUnloadEvent, ClientModifiers, ClientTickPostEvent, Flow, Hooks,
    KeyConflictContext, KeyRegistration, LightLayer, Registrar, RegistrationContext,
    RenderLevelStage, RenderLevelStageEvent, ScreenCharTypedPreEvent, ScreenClosingEvent,
    ScreenKeyPressedPreEvent, ScreenMouseButtonPressedPreEvent, ScreenRenderPostEvent, Text,
    WorldDepth, WorldLine,
};

pub mod chunks;
pub mod light;
pub mod search;

/// This mod's id, as `horizon-mod.toml` gives it: the owner of its key mappings.
const MOD_ID: &str = "moreoverlays";
const KEY_LIGHT: &str = "lightoverlay";
const KEY_CHUNKS: &str = "chunkbounds";
const KEY_SEARCH: &str = "invsearch";

/// How far the light overlay looks around the player, in blocks.
const LIGHT_RADIUS: i32 = 16;
/// How far up and down it looks.
const LIGHT_HEIGHT: i32 = 8;
/// The light overlay is rebuilt every this many client ticks.
const LIGHT_REFRESH_TICKS: u32 = 10;

/// GLFW's modifier bits a key mapping can hold (shift, control, alt, super).
const MODIFIER_BITS: u32 = 1 | 2 | 4 | 8;

/// The open search, while a container screen shows it.
struct Search {
    widgets: Widgets,
    field: WidgetId,
}

impl Search {
    fn query(&self) -> &str {
        self.widgets
            .text_field(self.field)
            .map_or("", |field| field.value())
    }
}

#[derive(Default)]
struct State {
    light_on: bool,
    chunk_mode: chunks::Mode,
    ticks: u32,
    /// The light overlay's crosses: the open space and its mark.
    crosses: Vec<([i32; 3], light::Mark)>,
    /// The player's feet and the world's height, for the chunk lines.
    feet: Option<[i32; 3]>,
    height: (i32, i32),
    search: Option<Search>,
}

#[derive(Default)]
struct Mod(Mutex<State>);

impl Mod {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(|poison| poison.into_inner())
    }
}

/// Rebuild the light overlay's crosses around the local player.
fn scan_light(host: &dyn ClientHost, feet: [i32; 3]) -> Vec<([i32; 3], light::Mark)> {
    let Some(level) = host.level(host.dimension()) else {
        return Vec::new();
    };
    let limits = level
        .dimension_type(host)
        .map_or(light::Limits::OVERWORLD, |dim| light::Limits {
            block_limit: dim.monster_spawn_block_light_limit,
            max_level: dim.monster_spawn_light_level.map_or(7, |(_, max)| max),
            has_sky: dim.has_skylight,
        });
    let prevents = |state| {
        host.block_state_string(state)
            .and_then(|name| {
                let id = name.split('[').next()?.to_owned();
                host.block_key(&id)
            })
            .and_then(|block| host.block_in_tag(block, "minecraft:prevent_mob_spawning_inside"))
            .unwrap_or(false)
    };
    let mut out = Vec::new();
    for dx in -LIGHT_RADIUS..=LIGHT_RADIUS {
        for dz in -LIGHT_RADIUS..=LIGHT_RADIUS {
            for dy in -LIGHT_HEIGHT..=LIGHT_HEIGHT {
                let space = [feet[0] + dx, feet[1] + dy, feet[2] + dz];
                let below_pos = [space[0], space[1] - 1, space[2]];
                let (Some(space_state), Some(below_state)) = (
                    level.block_state(host, space),
                    level.block_state(host, below_pos),
                ) else {
                    continue;
                };
                let Some(below) = host.block_state_string(below_state) else {
                    continue;
                };
                let below_shape = host.state_outline(below_state).unwrap_or_default();
                if !light::full_top(&below_shape) {
                    continue;
                }
                let space_collision = host.block_collision(space);
                let spot = light::Spot {
                    below: &below,
                    below_shape: &below_shape,
                    space_collision: &space_collision,
                    space_fluid: host
                        .state_fluid_state(space_state)
                        .is_some_and(|fluid| !fluid.is_empty()),
                    space_prevents: prevents(space_state),
                };
                if !light::spawnable(&spot) {
                    continue;
                }
                // The client's light: until Horizon reads it on a client (README, "Needs from
                // Horizon"), this answers only where the window is also the server.
                let (Some(block), Some(sky)) = (
                    level.brightness(host, LightLayer::Block, space),
                    level.brightness(host, LightLayer::Sky, space),
                ) else {
                    continue;
                };
                if let Some(mark) = light::classify(block, sky, limits) {
                    out.push((space, mark));
                }
            }
        }
    }
    out
}

/// Whether a screen key press is the search key mapping as the player bound it.
fn is_search_key(host: &dyn ClientHost, key: &str, modifiers: u32) -> bool {
    host.mod_key(MOD_ID, KEY_SEARCH)
        .is_some_and(|(bound, mods)| {
            bound == key && (modifiers & MODIFIER_BITS) == (mods.bits & MODIFIER_BITS)
        })
}

/// A container screen the search works on: every container, station and the survival
/// inventory report `inventory`; creative has its own search.
fn searchable(screen_id: &str, slots: usize) -> bool {
    screen_id == "inventory" && slots > 0
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let category = "key.categories.moreoverlays";
        r.register_key_mapping(
            KeyRegistration::new(KEY_LIGHT, "key.keyboard.f7")
                .label("key.moreoverlays.lightoverlay")
                .category(category)
                .context(KeyConflictContext::InGame),
        );
        r.register_key_mapping(
            KeyRegistration::new(KEY_CHUNKS, "key.keyboard.f9")
                .label("key.moreoverlays.chunkbounds")
                .category(category)
                .context(KeyConflictContext::InGame),
        );
        r.register_key_mapping(
            KeyRegistration::new(KEY_SEARCH, "key.keyboard.f")
                .label("key.moreoverlays.invsearch")
                .category(category)
                .modifiers(ClientModifiers::new(2))
                .context(KeyConflictContext::Gui),
        );
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        let light_pressed = host.consume_click(MOD_ID, KEY_LIGHT);
        let chunks_pressed = host.consume_click(MOD_ID, KEY_CHUNKS);
        let feet = host
            .local_player()
            .and_then(|player| player.entity().position(&*host))
            .map(|pos| pos.map(|axis| axis.floor() as i32));
        let height = host
            .level(host.dimension())
            .and_then(|level| level.height(&*host))
            .map(|(min, height)| (min, min + height));
        let rescan = {
            let mut state = self.state();
            if light_pressed {
                state.light_on = !state.light_on;
                state.crosses.clear();
                state.ticks = 0;
            }
            if chunks_pressed {
                state.chunk_mode = state.chunk_mode.next();
            }
            state.feet = feet;
            if let Some(height) = height {
                state.height = height;
            }
            let due = state.ticks == 0;
            state.ticks = (state.ticks + 1) % LIGHT_REFRESH_TICKS;
            state.light_on && due
        };
        if rescan && let Some(feet) = feet {
            let crosses = scan_light(&*host, feet);
            self.state().crosses = crosses;
        }
    }

    fn client_level_unload(&self, _: &mut dyn ClientHost, _: &mut ClientLevelUnloadEvent<'_>) {
        let mut state = self.state();
        state.crosses.clear();
        state.feet = None;
    }

    fn render_level_stage(&self, _: &dyn ClientHost, event: &mut RenderLevelStageEvent<'_>) {
        if event.stage != RenderLevelStage::AfterLevel {
            return;
        }
        let state = self.state();
        let mut lines: Vec<WorldLine> = Vec::new();
        if let Some(feet) = state.feet {
            let (min_y, max_y) = state.height;
            for (from, to, color) in chunks::lines(state.chunk_mode, feet, min_y, max_y) {
                lines.push(WorldLine::new(from, to, color, 2.0, WorldDepth::Tested));
            }
        }
        if state.light_on {
            for (space, mark) in &state.crosses {
                for (from, to) in light::cross(*space) {
                    lines.push(WorldLine::new(
                        from,
                        to,
                        mark.color(),
                        2.0,
                        WorldDepth::Tested,
                    ));
                }
            }
        }
        if lines.is_empty() {
            return;
        }
        // Shared with every other mod's lines: keep to what is left of the view's budget.
        let usage = event.draw.usage();
        let room = usage.line_capacity.saturating_sub(usage.lines) as usize;
        lines.truncate(room);
        let _ = event.draw.lines(&lines);
    }

    fn screen_key_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenKeyPressedPreEvent<'_>,
    ) -> Flow {
        if !searchable(event.screen.id, event.screen.slots) {
            return Flow::Continue;
        }
        let mut state = self.state();
        if is_search_key(host, event.key, event.modifiers) {
            if state.search.take().is_none() {
                let mut widgets = Widgets::new();
                let mut field = TextField::new(search::field_rect(event.screen.rect), 50);
                field.message = host
                    .text_string(&Text::translate(
                        "gui.moreoverlays.search.message",
                        Vec::new(),
                    ))
                    .unwrap_or_default();
                let id = widgets.add_text_field(field);
                widgets.set_focus(host, Some(id));
                state.search = Some(Search { widgets, field: id });
            }
            return Flow::Cancel;
        }
        let Some(search) = state.search.as_mut() else {
            return Flow::Continue;
        };
        if search.widgets.focused().is_none() || event.key == "key.keyboard.escape" {
            return Flow::Continue;
        }
        // While the field has the focus it takes every key but Escape, so typing `e` or a
        // hotbar number doesn't close the screen or move items.
        let _ = search.widgets.key_pressed(host, event.key, event.modifiers);
        Flow::Cancel
    }

    fn screen_char_typed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenCharTypedPreEvent<'_>,
    ) -> Flow {
        let mut state = self.state();
        let typed = state
            .search
            .as_mut()
            .is_some_and(|search| search.widgets.char_typed(host, event.c).handled());
        if typed { Flow::Cancel } else { Flow::Continue }
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        let mut state = self.state();
        let Some(search) = state.search.as_mut() else {
            return Flow::Continue;
        };
        if search
            .widgets
            .mouse_pressed(host, event.screen.mouse, event.button, event.modifiers)
            .handled()
        {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn screen_closing(&self, _: &dyn ClientHost, _: &mut ScreenClosingEvent<'_>) {
        self.state().search = None;
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        if !searchable(event.screen.id, event.screen.slots) {
            return;
        }
        let mut state = self.state();
        let Some(search) = state.search.as_mut() else {
            return;
        };
        let query = search.query().to_owned();
        if search::active(&query)
            && let Some(menu) = host
                .local_player()
                .and_then(|player| player.container_menu(host))
            && let Some(boxes) = menu.slot_boxes(host)
        {
            for (slot, rect) in boxes.into_iter().enumerate() {
                let Some(rect) = rect else { continue };
                let shown = menu
                    .slot(host, slot)
                    .flatten()
                    .is_some_and(|stack| search::matches(&query, &host.hover_name(&stack)));
                if !shown {
                    event.draw.fill(rect, search::DIM);
                }
            }
        }
        let _ = search.widgets.draw(event.draw, host, event.screen.mouse);
    }
}
