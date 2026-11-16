//! Controlling for Horizon: a searchable, filterable list of the key bindings.
//!
//! Written from Controlling's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). Controlling adds a search box and filter buttons to vanilla's
//! Key Binds screen. Horizon's mod API cannot change that screen nor rebind a key (README, "Needs
//! from Horizon"), so this port opens the same list as its own screen: from a "Search keys"
//! button on the options screens, or from its own key. The list logic is in `search.rs`.
use std::sync::Mutex;

use mod_api::client::widgets::{Button, Outcome, TextField, WidgetId, Widgets};
use mod_api::{
    ClientHost, ClientTickPostEvent, Flow, Hooks, KeyRegistration, Registrar, RegistrationContext,
    ScreenCharTypedPreEvent, ScreenKeyPressedPreEvent, ScreenMouseButtonPressedPreEvent,
    ScreenMouseScrolledPreEvent, ScreenRenderPostEvent, Text,
};

pub mod keys;
pub mod search;

use search::{Binding, Query};

/// This mod's id, as `horizon-mod.toml` names it.
pub const MOD_ID: &str = "controlling";
/// A row of the list, in GUI pixels.
const ROW: f32 = 14.0;
/// The list's top edge.
const LIST_TOP: f32 = 72.0;
/// The "Search keys" button drawn on the options screens: its size, from the top right corner.
const OPEN_BUTTON: [f32; 2] = [100.0, 20.0];

/// The widgets of the screen, by role.
struct Ids {
    search: WidgetId,
    show: WidgetId,
    by: WidgetId,
    sort: WidgetId,
    free: WidgetId,
    done: WidgetId,
}

#[derive(Default)]
struct State {
    query: Query,
    /// The widgets, built for `size`.
    widgets: Option<(Widgets, Ids)>,
    size: [f32; 2],
    /// How far the list is scrolled, in pixels.
    scroll: f32,
    /// Showing the keys no binding uses instead of the bindings.
    free_keys: bool,
    /// Asked from a screen hook, done on the next client tick (which may open screens).
    open: bool,
    close: bool,
}

fn tr(host: &dyn ClientHost, key: &str) -> String {
    host.translate(key).unwrap_or_else(|| key.to_owned())
}

fn label(key: &str, value: &str) -> Text {
    Text::translate(key, vec![Text::translate(value, Vec::new())])
}

/// The bindings as the game has them now.
fn read_bindings(host: &dyn ClientHost) -> Vec<Binding> {
    let not_bound = tr(host, "key.keyboard.unknown");
    keys::MAPPINGS
        .iter()
        .filter_map(|&(id, category)| {
            let mapping = host.mapping(id).ok().flatten()?;
            let key = host.text_string(&mapping.label).ok()?;
            Some(Binding {
                id: id.to_owned(),
                name: tr(host, id),
                category: category.to_owned(),
                category_name: tr(host, &format!("key.categories.{category}")),
                unbound: key.is_empty() || key == not_bound,
                key,
            })
        })
        .collect()
}

impl State {
    fn build(&mut self, size: [f32; 2]) {
        if self.widgets.is_some() && self.size == size {
            return;
        }
        let text = self.query.text.clone();
        let x = (size[0] / 2.0 - 155.0).floor();
        let mut w = Widgets::new();
        let mut field = TextField::new([x, 22.0, 310.0, 20.0], 64);
        field.set_value(&text);
        let search = w.add_text_field(field);
        let show = w.add_button(Button::new([x, 46.0, 100.0, 20.0], Text::literal("")));
        let by = w.add_button(Button::new(
            [x + 105.0, 46.0, 100.0, 20.0],
            Text::literal(""),
        ));
        let sort = w.add_button(Button::new(
            [x + 210.0, 46.0, 100.0, 20.0],
            Text::literal(""),
        ));
        let bottom = size[1] - 26.0;
        let free = w.add_button(Button::new([x, bottom, 100.0, 20.0], Text::literal("")));
        // Controlling's "Reset All": it needs a way to rebind (README), so it stays greyed out.
        let mut reset = Button::new(
            [x + 105.0, bottom, 100.0, 20.0],
            Text::translate("controls.resetAll", Vec::new()),
        );
        reset.active = false;
        w.add_button(reset);
        let done = w.add_button(Button::new(
            [x + 210.0, bottom, 100.0, 20.0],
            Text::translate("gui.done", Vec::new()),
        ));
        self.widgets = Some((
            w,
            Ids {
                search,
                show,
                by,
                sort,
                free,
                done,
            },
        ));
        self.size = size;
        self.relabel();
    }

    /// Puts the current choices on the buttons.
    fn relabel(&mut self) {
        let Some((w, ids)) = &mut self.widgets else {
            return;
        };
        let labels = [
            (ids.show, label("controlling.show", self.query.show.label())),
            (
                ids.by,
                label("controlling.search_by", self.query.by.label()),
            ),
            (ids.sort, label("controlling.sort", self.query.sort.label())),
            (
                ids.free,
                Text::translate(
                    if self.free_keys {
                        "controlling.bindings"
                    } else {
                        "controlling.free_keys"
                    },
                    Vec::new(),
                ),
            ),
        ];
        for (id, text) in labels {
            if let Some(button) = w.button_mut(id) {
                button.label = text;
            }
        }
    }

    /// Acts on what a widget did.
    fn act(&mut self, outcome: Outcome) {
        let Some((w, ids)) = &self.widgets else {
            return;
        };
        match outcome {
            Outcome::Changed(id) if id == ids.search => {
                self.query.text = w
                    .text_field(id)
                    .map(|f| f.value().to_owned())
                    .unwrap_or_default();
                self.scroll = 0.0;
            }
            Outcome::Pressed(id) => {
                if id == ids.show {
                    self.query.show = self.query.show.next();
                } else if id == ids.by {
                    self.query.by = self.query.by.next();
                } else if id == ids.sort {
                    self.query.sort = self.query.sort.next();
                } else if id == ids.free {
                    self.free_keys = !self.free_keys;
                } else if id == ids.done {
                    self.close = true;
                }
                self.scroll = 0.0;
                self.relabel();
            }
            _ => {}
        }
    }

    /// The lines the list draws: a category heading (`true`) or a binding, with its colour.
    fn lines(&self, host: &dyn ClientHost) -> Vec<(String, String, u32, bool)> {
        let bindings = read_bindings(host);
        if self.free_keys {
            let names: Vec<(String, String)> = keys::keyboard_keys()
                .into_iter()
                .map(|id| {
                    let name = tr(host, &id);
                    (id, name)
                })
                .collect();
            return search::free_keys(&names, &bindings)
                .into_iter()
                .map(|id| (tr(host, id), String::new(), 0xFFFF_FFFF, false))
                .collect();
        }
        let clash = search::conflicts(&bindings);
        let mut lines = Vec::new();
        let mut category = "";
        for i in search::filter(&bindings, &self.query) {
            let b = &bindings[i];
            if self.query.sort == search::Sort::None && b.category != category {
                category = &b.category;
                lines.push((b.category_name.clone(), String::new(), 0xFFFF_FFFF, true));
            }
            let color = if clash[i] { 0xFFFF_5555 } else { 0xFFFF_FFFF };
            lines.push((b.name.clone(), b.key.clone(), color, false));
        }
        lines
    }
}

/// Whether the screen being drawn is this mod's own.
fn ours(host: &dyn ClientHost, id: &str) -> bool {
    id == "mod" && host.mod_screen() == Some(MOD_ID)
}

fn open_button(size: [f32; 2]) -> [f32; 4] {
    [
        size[0] - OPEN_BUTTON[0] - 4.0,
        4.0,
        OPEN_BUTTON[0],
        OPEN_BUTTON[1],
    ]
}

fn inside(rect: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= rect[0] && at[0] < rect[0] + rect[2] && at[1] >= rect[1] && at[1] < rect[1] + rect[3]
}

#[derive(Default)]
struct Mod {
    state: Mutex<State>,
}

impl Mod {
    fn with<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut state)
    }
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        r.register_key_mapping(
            KeyRegistration::new("open", "key.keyboard.unknown")
                .label("key.controlling.open")
                .category("key.categories.controlling"),
        );
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        let clicked = host.consume_click(MOD_ID, "open");
        let (open, close) = self.with(|s| {
            let asked = (s.open || clicked, s.close);
            s.open = false;
            s.close = false;
            asked
        });
        if close {
            host.mod_screen_close(MOD_ID);
        }
        if open {
            // From the options screens: close them first, as a mod screen opens over the world.
            for _ in 0..8 {
                if host.mod_screen_open(MOD_ID) || !host.close_screen() {
                    break;
                }
            }
        }
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        let size = event.draw.size();
        if event.screen.id == "options" && host.local_player().is_some() {
            let _ = event.draw.widget_button(
                open_button(size),
                &Text::translate("controlling.open", Vec::new()),
                true,
                inside(open_button(size), event.screen.mouse),
            );
            return;
        }
        if !ours(host, event.screen.id) {
            return;
        }
        self.with(|s| {
            s.build(size);
            let draw = &mut *event.draw;
            draw.fill([0.0, 0.0, size[0], size[1]], 0xC010_1010);
            let title = tr(host, "controlling.title");
            let tw = draw.text_width(&title);
            draw.text(&title, [(size[0] - tw) / 2.0, 8.0], 0xFFFF_FFFF, true);
            let lines = s.lines(host);
            let bottom = size[1] - 32.0;
            let max_scroll = (lines.len() as f32 * ROW - (bottom - LIST_TOP)).max(0.0);
            s.scroll = s.scroll.clamp(0.0, max_scroll);
            let left = (size[0] / 2.0 - 155.0).floor();
            let _ = draw.push_clip([0.0, LIST_TOP, size[0], bottom - LIST_TOP]);
            for (i, (name, key, color, heading)) in lines.iter().enumerate() {
                let y = LIST_TOP + i as f32 * ROW - s.scroll;
                if y + ROW < LIST_TOP || y > bottom {
                    continue;
                }
                if *heading {
                    let hw = draw.text_width(name);
                    draw.text(name, [(size[0] - hw) / 2.0, y + 3.0], 0xFFFF_FFFF, false);
                } else {
                    draw.text(name, [left, y + 3.0], 0xFFFF_FFFF, false);
                    let kw = draw.text_width(key);
                    draw.text(key, [left + 310.0 - kw, y + 3.0], *color, false);
                }
            }
            let _ = draw.pop_clip();
            if lines.is_empty() {
                let none = tr(host, "controlling.empty");
                let nw = draw.text_width(&none);
                draw.text(
                    &none,
                    [(size[0] - nw) / 2.0, LIST_TOP + 4.0],
                    0xFFA0_A0A0,
                    false,
                );
            }
            if let Some((w, _)) = &mut s.widgets {
                let _ = w.draw(draw, host, event.screen.mouse);
            }
        });
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        let at = event.screen.mouse;
        if event.screen.id == "options" && host.local_player().is_some() {
            // The options screens have no panel: `rect` is the whole window.
            let size = [event.screen.rect[2], event.screen.rect[3]];
            if event.button == 0 && inside(open_button(size), at) {
                self.with(|s| s.open = true);
                return Flow::Cancel;
            }
            return Flow::Continue;
        }
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        self.with(|s| {
            let Some((w, _)) = &mut s.widgets else {
                return Flow::Continue;
            };
            let outcome = w.mouse_pressed(host, at, event.button, event.modifiers);
            s.act(outcome);
            if outcome.handled() {
                Flow::Cancel
            } else {
                Flow::Continue
            }
        })
    }

    fn screen_mouse_scrolled_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseScrolledPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        self.with(|s| s.scroll -= event.scroll[1] as f32 * ROW * 2.0);
        Flow::Cancel
    }

    fn screen_key_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenKeyPressedPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        self.with(|s| {
            let Some((w, _)) = &mut s.widgets else {
                return Flow::Continue;
            };
            let outcome = w.key_pressed(host, event.key, event.modifiers);
            s.act(outcome);
            if outcome.handled() {
                Flow::Cancel
            } else {
                Flow::Continue
            }
        })
    }

    fn screen_char_typed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenCharTypedPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        self.with(|s| {
            let Some((w, _)) = &mut s.widgets else {
                return Flow::Continue;
            };
            let outcome = w.char_typed(host, event.c);
            s.act(outcome);
            if outcome.handled() {
                Flow::Cancel
            } else {
                Flow::Continue
            }
        })
    }
}
