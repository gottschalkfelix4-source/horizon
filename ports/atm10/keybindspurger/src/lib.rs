//! KeybindsPurger for Horizon: purge the key bindings you never use, so their keys are free.
//!
//! Written from KeybindsPurger's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). The player marks bindings as purged on this mod's screen (or
//! lets it mark every binding that clashes with an earlier one); the list is saved in the mod's
//! data folder with the key each had. A purge unbinds the binding and hides it from Controls
//! (`set_key_binding`, `set_key_binding_hidden`); keeping it again restores its key. The choices are
//! in `purge.rs`.
use std::sync::Mutex;

use mod_api::client::widgets::{Button, Outcome, WidgetId, Widgets};
use mod_api::{
    ClientHost, ClientModifiers, ClientTickPostEvent, ConfigPath, Flow, Hooks, KeyRegistration,
    Registrar, RegistrationContext, ScreenMouseButtonPressedPreEvent, ScreenMouseScrolledPreEvent,
    ScreenRenderPostEvent, Text,
};

pub mod purge;

use purge::{Binding, Purged, Step};

/// This mod's id, as `horizon-mod.toml` names it.
pub const MOD_ID: &str = "keybindspurger";
/// The saved list, in the mod's data folder.
const LIST_FILE: &str = "purged.txt";
const ROW: f32 = 14.0;
const LIST_TOP: f32 = 30.0;

/// What the player asked on the screen, done on the next client tick (the screen hooks hold the
/// host read-only).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Action {
    Toggle(String),
    PurgeConflicts,
    PurgeModded,
    KeepAll,
}

/// How often purged bindings are checked for a key the player gave them since, in client ticks.
const ENFORCE_EVERY: u32 = 40;

struct Ids {
    conflicts: WidgetId,
    modded: WidgetId,
    clear: WidgetId,
    done: WidgetId,
}

#[derive(Default)]
struct State {
    purged: Option<Purged>,
    widgets: Option<(Widgets, Ids)>,
    size: [f32; 2],
    scroll: f32,
    open: bool,
    close: bool,
    actions: Vec<Action>,
    ticks: u32,
}

fn tr(host: &dyn ClientHost, key: &str) -> String {
    host.translate(key).unwrap_or_else(|| key.to_owned())
}

/// The bindings as the game has them now, in Controls' order, with the name and key Controls shows.
fn read_bindings(host: &dyn ClientHost) -> Vec<(Binding, String, String)> {
    host.key_bindings()
        .into_iter()
        .map(|v| {
            let name = host.text_string(&v.label).unwrap_or_else(|_| v.id.clone());
            let key = tr(host, &v.key);
            (
                Binding {
                    id: v.id,
                    key_id: v.key,
                    modifiers: v.modifiers.bits,
                    default_key: v.default_key,
                    default_modifiers: v.default_modifiers.bits,
                    context: v.context,
                    owner: v.owner,
                },
                name,
                key,
            )
        })
        .collect()
}

fn plain(rows: &[(Binding, String, String)]) -> Vec<Binding> {
    rows.iter().map(|(b, ..)| b.clone()).collect()
}

fn apply(host: &mut dyn ClientHost, steps: Vec<Step>) {
    for step in steps {
        match step {
            Step::Rebind(id, key, modifiers) => {
                let _ = host.set_key_binding(&id, &key, ClientModifiers::new(modifiers));
            }
            Step::Hide(id, hidden) => host.set_key_binding_hidden(&id, hidden),
        }
    }
}

fn path() -> Option<ConfigPath> {
    ConfigPath::new(LIST_FILE).ok()
}

impl State {
    fn purged(&mut self, host: &dyn ClientHost) -> &mut Purged {
        self.purged.get_or_insert_with(|| {
            let text = path()
                .and_then(|p| host.data_files().read(&p).ok())
                .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                .unwrap_or_default();
            Purged::parse(&text)
        })
    }

    fn save(&mut self, host: &dyn ClientHost) {
        let text = self.purged(host).write();
        if let Some(p) = path() {
            let _ = host.data_files().write(&p, text.as_bytes());
        }
    }

    fn build(&mut self, size: [f32; 2]) {
        if self.widgets.is_some() && self.size == size {
            return;
        }
        let x = (size[0] / 2.0 - 155.0).floor();
        let y = size[1] - 26.0;
        let mut w = Widgets::new();
        let conflicts = w.add_button(Button::new(
            [x, y, 76.0, 20.0],
            Text::translate("keybindspurger.purge_conflicts", Vec::new()),
        ));
        let modded = w.add_button(Button::new(
            [x + 78.0, y, 76.0, 20.0],
            Text::translate("keybindspurger.purge_modded", Vec::new()),
        ));
        let clear = w.add_button(Button::new(
            [x + 156.0, y, 76.0, 20.0],
            Text::translate("keybindspurger.clear", Vec::new()),
        ));
        let done = w.add_button(Button::new(
            [x + 234.0, y, 76.0, 20.0],
            Text::translate("gui.done", Vec::new()),
        ));
        self.widgets = Some((
            w,
            Ids {
                conflicts,
                modded,
                clear,
                done,
            },
        ));
        self.size = size;
    }
}

fn ours(host: &dyn ClientHost, id: &str) -> bool {
    id == "mod" && host.mod_screen() == Some(MOD_ID)
}

fn open_button() -> [f32; 4] {
    [4.0, 4.0, 100.0, 20.0]
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

impl Mod {
    /// Does what the screen asked, and every so often unbinds the purged bindings the player has bound
    /// again (a mod's, registered after the purge, included).
    fn update_bindings(&self, host: &mut dyn ClientHost, actions: Vec<Action>) {
        let due = self.with(|s| {
            s.ticks = s.ticks.wrapping_add(1);
            !actions.is_empty() || s.ticks % ENFORCE_EVERY == 1
        });
        if !due {
            return;
        }
        let rows = read_bindings(host);
        let bindings = plain(&rows);
        let steps = self.with(|s| {
            let purged = s.purged(host);
            let mut steps = Vec::new();
            for action in actions {
                steps.extend(match action {
                    Action::Toggle(id) => match bindings.iter().find(|b| b.id == id) {
                        Some(b) if purged.contains(&id) => purged.keep(b),
                        Some(b) => purged.purge(b),
                        None => Vec::new(),
                    },
                    Action::PurgeConflicts => purged.purge_conflicts(&bindings),
                    Action::PurgeModded => purged.purge_modded(&bindings, MOD_ID),
                    Action::KeepAll => purged.keep_all(&bindings),
                });
            }
            let changed = !steps.is_empty();
            // What is already unbound needs no second unbinding.
            let extra: Vec<Step> = purged
                .enforce(&bindings)
                .into_iter()
                .filter(|e| !steps.contains(e))
                .collect();
            steps.extend(extra);
            if changed {
                s.save(host);
            }
            steps
        });
        apply(host, steps);
    }
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        r.register_key_mapping(
            KeyRegistration::new("open", "key.keyboard.unknown")
                .label("key.keybindspurger.open")
                .category("key.categories.keybindspurger"),
        );
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        let clicked = host.consume_click(MOD_ID, "open");
        let (open, close, actions) = self.with(|s| {
            let asked = (s.open || clicked, s.close, std::mem::take(&mut s.actions));
            s.open = false;
            s.close = false;
            asked
        });
        self.update_bindings(host, actions);
        if close {
            host.mod_screen_close(MOD_ID);
        }
        if open {
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
                open_button(),
                &Text::translate("keybindspurger.open", Vec::new()),
                true,
                inside(open_button(), event.screen.mouse),
            );
            return;
        }
        if !ours(host, event.screen.id) {
            return;
        }
        let bindings = read_bindings(host);
        self.with(|s| {
            s.build(size);
            let draw = &mut *event.draw;
            draw.fill([0.0, 0.0, size[0], size[1]], 0xC010_1010);
            let title = tr(host, "keybindspurger.title");
            let tw = draw.text_width(&title);
            draw.text(&title, [(size[0] - tw) / 2.0, 8.0], 0xFFFF_FFFF, true);
            let bottom = size[1] - 32.0;
            let max_scroll = (bindings.len() as f32 * ROW - (bottom - LIST_TOP)).max(0.0);
            s.scroll = s.scroll.clamp(0.0, max_scroll);
            let left = (size[0] / 2.0 - 155.0).floor();
            let purged_word = tr(host, "keybindspurger.purged");
            let scroll = s.scroll;
            let purged = s.purged(host).clone();
            let _ = draw.push_clip([0.0, LIST_TOP, size[0], bottom - LIST_TOP]);
            for (i, (b, name, key)) in bindings.iter().enumerate() {
                let y = LIST_TOP + i as f32 * ROW - scroll;
                if y + ROW < LIST_TOP || y > bottom {
                    continue;
                }
                let gone = purged.contains(&b.id);
                if inside([left, y, 310.0, ROW], event.screen.mouse) {
                    draw.fill([left - 2.0, y, 314.0, ROW], 0x40FF_FFFF);
                }
                let name_color = if gone { 0xFF80_8080 } else { 0xFFFF_FFFF };
                draw.text(name, [left, y + 3.0], name_color, false);
                let right = if gone {
                    purged_word.clone()
                } else {
                    key.clone()
                };
                let color = if gone { 0xFFFF_5555 } else { 0xFFA0_FFA0 };
                let rw = draw.text_width(&right);
                draw.text(&right, [left + 310.0 - rw, y + 3.0], color, false);
            }
            let _ = draw.pop_clip();
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
            if event.button == 0 && inside(open_button(), at) {
                self.with(|s| s.open = true);
                return Flow::Cancel;
            }
            return Flow::Continue;
        }
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        let bindings = read_bindings(host);
        self.with(|s| {
            let Some((w, ids)) = &mut s.widgets else {
                return Flow::Continue;
            };
            let (conflicts, modded, clear, done) = (ids.conflicts, ids.modded, ids.clear, ids.done);
            let outcome = w.mouse_pressed(host, at, event.button, event.modifiers);
            match outcome {
                Outcome::Pressed(id) if id == conflicts => s.actions.push(Action::PurgeConflicts),
                Outcome::Pressed(id) if id == modded => s.actions.push(Action::PurgeModded),
                Outcome::Pressed(id) if id == clear => s.actions.push(Action::KeepAll),
                Outcome::Pressed(id) if id == done => s.close = true,
                Outcome::Ignored if event.button == 0 => {
                    let left = (s.size[0] / 2.0 - 155.0).floor();
                    let bottom = s.size[1] - 32.0;
                    if at[0] >= left && at[0] < left + 310.0 && at[1] >= LIST_TOP && at[1] < bottom
                    {
                        let row = ((at[1] - LIST_TOP + s.scroll) / ROW) as usize;
                        if let Some((b, ..)) = bindings.get(row) {
                            s.actions.push(Action::Toggle(b.id.clone()));
                            return Flow::Cancel;
                        }
                    }
                }
                _ => {}
            }
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
}
