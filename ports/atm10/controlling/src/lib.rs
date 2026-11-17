//! Controlling for Horizon: a searchable, filterable list of the key bindings.
//!
//! Written from Controlling's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). Controlling adds a search box and filter buttons to vanilla's
//! Key Binds screen. Horizon's mod API cannot change that screen (README, "Needs from Horizon"),
//! so this port opens the same list as its own screen, with rebinding: from a "Search keys"
//! button on the options screens, or from its own key. The list logic is in `search.rs`.
use std::sync::Mutex;

use mod_api::ClientModifiers;
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

/// The width of the name column, then the key button and the per-row Reset button.
const NAME_W: f32 = 150.0;
const KEY_X: f32 = 154.0;
const KEY_W: f32 = 100.0;
const RESET_X: f32 = 258.0;
const RESET_W: f32 = 52.0;

/// A change of the game's bindings asked from a screen hook, made on the next client tick (the hooks
/// hold the host read-only).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Change {
    Set(String, String, u32),
    ResetAll,
}

/// The widgets of the screen, by role.
struct Ids {
    search: WidgetId,
    show: WidgetId,
    by: WidgetId,
    sort: WidgetId,
    free: WidgetId,
    reset_all: WidgetId,
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
    /// The binding waiting for its new key (the player clicked its key button).
    listening: Option<String>,
    /// Binding changes waiting for the next client tick.
    changes: Vec<Change>,
}

/// One row of the list.
struct Line {
    /// The binding's id; `None` for a category heading.
    id: Option<String>,
    name: String,
    key: String,
    color: u32,
    heading: bool,
    /// Whether Reset has something to put back.
    changed: bool,
}

fn tr(host: &dyn ClientHost, key: &str) -> String {
    host.translate(key).unwrap_or_else(|| key.to_owned())
}

fn label(key: &str, value: &str) -> Text {
    Text::translate(key, vec![Text::translate(value, Vec::new())])
}

/// The bindings as the game has them now: vanilla's, Horizon's and every mod's.
fn read_bindings(host: &dyn ClientHost) -> Vec<Binding> {
    host.key_bindings()
        .into_iter()
        .map(|v| {
            let category_name = if v.category.is_empty() {
                tr(host, "controlling.category.none")
            } else {
                tr(host, &v.category)
            };
            let key = search::key_label(&tr(host, &v.key), v.modifiers.bits, |m| {
                tr(host, &format!("controlling.modifier.{m}"))
            });
            Binding {
                name: host.text_string(&v.label).unwrap_or_else(|_| v.id.clone()),
                id: v.id,
                category: v.category,
                category_name,
                unbound: v.key == search::UNBOUND,
                key,
                key_id: v.key,
                modifiers: v.modifiers.bits,
                default_key: v.default_key,
                default_modifiers: v.default_modifiers.bits,
                context: v.context,
                owner: v.owner,
            }
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
        let reset_all = w.add_button(Button::new(
            [x + 105.0, bottom, 100.0, 20.0],
            Text::translate("controls.resetAll", Vec::new()),
        ));
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
                reset_all,
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
                } else if id == ids.reset_all {
                    self.changes.push(Change::ResetAll);
                } else if id == ids.done {
                    self.close = true;
                }
                self.scroll = 0.0;
                self.relabel();
            }
            _ => {}
        }
    }

    /// The lines the list draws.
    fn lines(&self, host: &dyn ClientHost) -> Vec<Line> {
        let bindings = read_bindings(host);
        if self.free_keys {
            let ids = keys::keyboard_keys();
            return search::free_keys(&ids, &bindings)
                .into_iter()
                .map(|id| Line {
                    id: None,
                    name: tr(host, id),
                    key: String::new(),
                    color: 0xFFFF_FFFF,
                    heading: false,
                    changed: false,
                })
                .collect();
        }
        let clash = search::conflicts(&bindings);
        let mut lines = Vec::new();
        let mut category = None;
        for i in search::filter(&bindings, &self.query) {
            let b = &bindings[i];
            if self.query.sort == search::Sort::None && category != Some(&b.category) {
                category = Some(&b.category);
                lines.push(Line {
                    id: None,
                    name: b.category_name.clone(),
                    key: String::new(),
                    color: 0xFFFF_FFFF,
                    heading: true,
                    changed: false,
                });
            }
            let listening = self.listening.as_deref() == Some(b.id.as_str());
            lines.push(Line {
                id: Some(b.id.clone()),
                name: b.name.clone(),
                key: if listening {
                    format!("> {} <", b.key)
                } else {
                    b.key.clone()
                },
                color: if listening {
                    0xFFFF_FF55
                } else if clash[i] {
                    0xFFFF_5555
                } else {
                    0xFFFF_FFFF
                },
                heading: false,
                changed: b.changed(),
            });
        }
        lines
    }

    /// The list row at `at`, with the part of it hit: the key button or the Reset button.
    fn row_at(&self, host: &dyn ClientHost, at: [f32; 2]) -> Option<(String, bool)> {
        let bottom = self.size[1] - 32.0;
        if at[1] < LIST_TOP || at[1] >= bottom {
            return None;
        }
        let left = (self.size[0] / 2.0 - 155.0).floor();
        let lines = self.lines(host);
        let line = lines.get(((at[1] - LIST_TOP + self.scroll) / ROW).floor() as usize)?;
        let id = line.id.clone()?;
        let dx = at[0] - left;
        if (KEY_X..KEY_X + KEY_W).contains(&dx) {
            Some((id, false))
        } else if line.changed && (RESET_X..RESET_X + RESET_W).contains(&dx) {
            Some((id, true))
        } else {
            None
        }
    }

    /// The player picked `key` for the binding being listened for.
    fn finish_listening(&mut self, host: &dyn ClientHost, key: &str, held: u32) {
        let Some(id) = self.listening.take() else {
            return;
        };
        let Some(binding) = read_bindings(host).into_iter().find(|b| b.id == id) else {
            return;
        };
        // Escape unbinds, as on Key Binds.
        let (key, modifiers) = if key == "key.keyboard.escape" {
            (search::UNBOUND.to_owned(), 0)
        } else {
            search::rebind(&binding, key, held)
        };
        self.changes.push(Change::Set(id, key, modifiers));
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
        let (open, close, changes) = self.with(|s| {
            let asked = (s.open || clicked, s.close, std::mem::take(&mut s.changes));
            s.open = false;
            s.close = false;
            asked
        });
        for change in changes {
            match change {
                Change::Set(id, key, modifiers) => {
                    let _ = host.set_key_binding(&id, &key, ClientModifiers::new(modifiers));
                }
                Change::ResetAll => {
                    for (id, key, modifiers) in search::reset_plan(&read_bindings(host)) {
                        let _ = host.set_key_binding(&id, &key, ClientModifiers::new(modifiers));
                    }
                }
            }
        }
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
            for (i, line) in lines.iter().enumerate() {
                let (name, key, color, heading) =
                    (&line.name, &line.key, &line.color, &line.heading);
                let y = LIST_TOP + i as f32 * ROW - s.scroll;
                if y + ROW < LIST_TOP || y > bottom {
                    continue;
                }
                if *heading {
                    let hw = draw.text_width(name);
                    draw.text(name, [(size[0] - hw) / 2.0, y + 3.0], 0xFFFF_FFFF, false);
                } else {
                    if line.id.is_none() {
                        draw.text(name, [left, y + 3.0], 0xFFFF_FFFF, false);
                        continue;
                    }
                    let _ = draw.push_clip([left, y, NAME_W, ROW]);
                    draw.text(name, [left, y + 3.0], 0xFFFF_FFFF, false);
                    let _ = draw.pop_clip();
                    let at = event.screen.mouse;
                    let key_rect = [left + KEY_X, y, KEY_W, ROW - 2.0];
                    let _ = draw.widget_button(
                        key_rect,
                        &Text::literal(""),
                        true,
                        inside(key_rect, at) || s.listening == line.id,
                    );
                    let kw = draw.text_width(key);
                    draw.text(
                        key,
                        [left + KEY_X + (KEY_W - kw) / 2.0, y + 2.0],
                        *color,
                        true,
                    );
                    if line.changed {
                        let rect = [left + RESET_X, y, RESET_W, ROW - 2.0];
                        let _ = draw.widget_button(
                            rect,
                            &Text::translate("controls.reset", Vec::new()),
                            true,
                            inside(rect, at),
                        );
                    }
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
            if s.listening.is_some() {
                // A mouse button picks the key, as on Key Binds.
                s.finish_listening(
                    host,
                    &search::mouse_key(event.button as u32),
                    event.modifiers,
                );
                return Flow::Cancel;
            }
            let Some((w, _)) = &mut s.widgets else {
                return Flow::Continue;
            };
            let outcome = w.mouse_pressed(host, at, event.button, event.modifiers);
            s.act(outcome);
            if outcome.handled() {
                return Flow::Cancel;
            }
            if event.button == 0 && !s.free_keys {
                match s.row_at(host, at) {
                    Some((id, false)) => {
                        s.listening = Some(id);
                        return Flow::Cancel;
                    }
                    Some((id, true)) => {
                        if let Some(b) = read_bindings(host).into_iter().find(|b| b.id == id) {
                            s.changes
                                .push(Change::Set(id, b.default_key, b.default_modifiers));
                        }
                        return Flow::Cancel;
                    }
                    None => {}
                }
            }
            Flow::Continue
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
            if s.listening.is_some() {
                s.finish_listening(host, event.key, event.modifiers);
                return Flow::Cancel;
            }
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
