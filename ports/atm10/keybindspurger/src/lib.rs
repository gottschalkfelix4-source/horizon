//! KeybindsPurger for Horizon: purge the key bindings you never use, so their keys are free.
//!
//! Written from KeybindsPurger's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). The player marks bindings as purged on this mod's screen (or
//! lets it mark every binding that clashes with an earlier one); the list is saved in the mod's
//! data folder. Horizon's mod API cannot unbind a key yet (README, "Needs from Horizon"): until it
//! can, a press is dropped when every binding on its key is purged, which frees the key for
//! nothing else but stops the purged actions. The choices are in `purge.rs`.
use std::sync::Mutex;

use mod_api::client::widgets::{Button, Outcome, WidgetId, Widgets};
use mod_api::{
    ClientHost, ClientTickPostEvent, ConfigPath, Flow, Hooks, KeyMappingPreEvent, KeyRegistration,
    Registrar, RegistrationContext, ScreenMouseButtonPressedPreEvent, ScreenMouseScrolledPreEvent,
    ScreenRenderPostEvent, Text,
};

pub mod keys;
pub mod purge;

use purge::{Binding, Purged};

/// This mod's id, as `horizon-mod.toml` names it.
pub const MOD_ID: &str = "keybindspurger";
/// The saved list, in the mod's data folder.
const LIST_FILE: &str = "purged.txt";
const ROW: f32 = 14.0;
const LIST_TOP: f32 = 30.0;

struct Ids {
    conflicts: WidgetId,
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
}

fn tr(host: &dyn ClientHost, key: &str) -> String {
    host.translate(key).unwrap_or_else(|| key.to_owned())
}

/// The bindings as the game has them now, in Controls' order.
fn read_bindings(host: &dyn ClientHost) -> Vec<Binding> {
    let not_bound = tr(host, "key.keyboard.unknown");
    keys::MAPPINGS
        .iter()
        .filter_map(|&(id, _)| {
            let mapping = host.mapping(id).ok().flatten()?;
            let key = host.text_string(&mapping.label).ok()?;
            Some(Binding {
                id: id.to_owned(),
                key: (!key.is_empty() && key != not_bound).then_some(key),
            })
        })
        .collect()
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
            [x, y, 100.0, 20.0],
            Text::translate("keybindspurger.purge_conflicts", Vec::new()),
        ));
        let clear = w.add_button(Button::new(
            [x + 105.0, y, 100.0, 20.0],
            Text::translate("keybindspurger.clear", Vec::new()),
        ));
        let done = w.add_button(Button::new(
            [x + 210.0, y, 100.0, 20.0],
            Text::translate("gui.done", Vec::new()),
        ));
        self.widgets = Some((
            w,
            Ids {
                conflicts,
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
            for _ in 0..8 {
                if host.mod_screen_open(MOD_ID) || !host.close_screen() {
                    break;
                }
            }
        }
    }

    /// A purged binding's press is dropped when no kept binding shares its key.
    fn key_mapping_pre(
        &self,
        host: &mut dyn ClientHost,
        event: &mut KeyMappingPreEvent<'_>,
    ) -> Flow {
        if !event.pressed {
            return Flow::Continue;
        }
        let host: &dyn ClientHost = host;
        let drop = self.with(|s| {
            if !s.purged(host).contains(event.mapping) {
                return false;
            }
            let bindings = read_bindings(host);
            s.purged(host).drops(event.mapping, &bindings)
        });
        if drop { Flow::Cancel } else { Flow::Continue }
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
            let not_bound = tr(host, "key.keyboard.unknown");
            let scroll = s.scroll;
            let purged = s.purged(host).clone();
            let _ = draw.push_clip([0.0, LIST_TOP, size[0], bottom - LIST_TOP]);
            for (i, b) in bindings.iter().enumerate() {
                let y = LIST_TOP + i as f32 * ROW - scroll;
                if y + ROW < LIST_TOP || y > bottom {
                    continue;
                }
                let gone = purged.contains(&b.id);
                if inside([left, y, 310.0, ROW], event.screen.mouse) {
                    draw.fill([left - 2.0, y, 314.0, ROW], 0x40FF_FFFF);
                }
                let name_color = if gone { 0xFF80_8080 } else { 0xFFFF_FFFF };
                draw.text(&tr(host, &b.id), [left, y + 3.0], name_color, false);
                let right = if gone {
                    purged_word.clone()
                } else {
                    b.key.clone().unwrap_or_else(|| not_bound.clone())
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
            let (conflicts, clear, done) = (ids.conflicts, ids.clear, ids.done);
            let outcome = w.mouse_pressed(host, at, event.button, event.modifiers);
            match outcome {
                Outcome::Pressed(id) if id == conflicts => {
                    s.purged(host).purge_conflicts(&bindings);
                    s.save(host);
                }
                Outcome::Pressed(id) if id == clear => {
                    *s.purged(host) = Purged::default();
                    s.save(host);
                }
                Outcome::Pressed(id) if id == done => s.close = true,
                Outcome::Ignored if event.button == 0 => {
                    let left = (s.size[0] / 2.0 - 155.0).floor();
                    let bottom = s.size[1] - 32.0;
                    if at[0] >= left && at[0] < left + 310.0 && at[1] >= LIST_TOP && at[1] < bottom
                    {
                        let row = ((at[1] - LIST_TOP + s.scroll) / ROW) as usize;
                        if let Some(b) = bindings.get(row) {
                            s.purged(host).toggle(&b.id);
                            s.save(host);
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
