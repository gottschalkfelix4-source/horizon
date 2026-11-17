//! KeyBind Bundles for Horizon: one key holds several key bindings, picked from a radial menu.
//!
//! Written from KeyBind Bundles' observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). Each bundle has its own key in Controls (`Bundle 1`...). Holding
//! it opens a radial menu of the bundle's bindings; letting go (or clicking) fires the binding
//! under the pointer as if its own key had been pressed (`press_binding`, so any binding of the game
//! or of a mod). A bundle with nothing in it opens the editor, where a click adds a binding to the
//! bundle or takes it out, and `+` adds a bundle (its key is registered at run time). The bundles
//! are saved in the mod's data folder. The model and the geometry are in `bundles.rs`.
use std::sync::Mutex;

use mod_api::client::widgets::{Button, Outcome, WidgetId, Widgets};
use mod_api::{
    ClientHost, ClientTickPostEvent, ConfigPath, Flow, Hooks, KeyRegistration, Registrar,
    RegistrationContext, ScreenKeyReleasedPreEvent, ScreenMouseButtonPressedPreEvent,
    ScreenMouseButtonReleasedPreEvent, ScreenMouseScrolledPreEvent, ScreenRenderPostEvent, Text,
};

pub mod bundles;

use bundles::{BUNDLES, Bundles, holdable};

/// This mod's id, as `horizon-mod.toml` names it.
pub const MOD_ID: &str = "keybindbundles";
const FILE: &str = "bundles.txt";
const INNER: f32 = 28.0;
const OUTER: f32 = 92.0;
const ROW: f32 = 14.0;
const LIST_TOP: f32 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// The radial menu of bundle `.0`.
    Radial(usize),
    /// The editor, on bundle `.0`.
    Editor(usize),
}

struct Ids {
    prev: WidgetId,
    next: WidgetId,
    add: WidgetId,
    done: WidgetId,
}

#[derive(Default)]
struct State {
    bundles: Option<Bundles>,
    mode: Option<Mode>,
    /// Asked by a hook without a mutable host; done on the next client tick.
    open: Option<Mode>,
    close: bool,
    fire: Option<String>,
    editor: Option<(Widgets, Ids, [f32; 2])>,
    /// How many bundles have their key registered (the first `BUNDLES` at startup).
    registered: usize,
    /// How far the editor's list is scrolled, in pixels.
    scroll: f32,
}

/// The bindings the editor lists, in Controls' order, as `(id, name)`: every one a bundle can hold.
fn editor_rows(host: &dyn ClientHost) -> Vec<(String, String)> {
    host.key_bindings()
        .into_iter()
        .filter(|v| holdable(&v.id))
        .map(|v| {
            let name = host.text_string(&v.label).unwrap_or_else(|_| v.id.clone());
            (v.id, name)
        })
        .collect()
}

/// A binding's name as Controls shows it, by id; the id itself when no mod provides it now.
fn names(host: &dyn ClientHost) -> std::collections::HashMap<String, String> {
    editor_rows(host).into_iter().collect()
}

fn bundle_key(n: usize) -> KeyRegistration {
    KeyRegistration::new(&key_id(n), "key.keyboard.unknown")
        .label(&format!("key.keybindbundles.bundle_{}", n + 1))
        .category("key.categories.keybindbundles")
}

fn key_id(n: usize) -> String {
    format!("bundle_{}", n + 1)
}

fn tr(host: &dyn ClientHost, key: &str) -> String {
    host.translate(key).unwrap_or_else(|| key.to_owned())
}

fn mouse_name(button: i32) -> String {
    match button {
        0 => "key.mouse.left".into(),
        1 => "key.mouse.right".into(),
        2 => "key.mouse.middle".into(),
        n => format!("key.mouse.{}", n + 1),
    }
}

fn inside(rect: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= rect[0] && at[0] < rect[0] + rect[2] && at[1] >= rect[1] && at[1] < rect[1] + rect[3]
}

impl State {
    fn bundles(&mut self, host: &dyn ClientHost) -> &mut Bundles {
        self.bundles.get_or_insert_with(|| {
            let text = ConfigPath::new(FILE)
                .ok()
                .and_then(|p| host.data_files().read(&p).ok())
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default();
            Bundles::parse(&text)
        })
    }

    fn save(&mut self, host: &dyn ClientHost) {
        let text = self.bundles(host).write();
        if let Ok(p) = ConfigPath::new(FILE) {
            let _ = host.data_files().write(&p, text.as_bytes());
        }
    }

    /// The radial menu is up and `input` (a key or mouse button name) is bundle's key: fire
    /// the slice under `at`, or close with nothing.
    fn release(
        &mut self,
        host: &dyn ClientHost,
        input: &str,
        at: [f32; 2],
        size: [f32; 2],
    ) -> bool {
        let Some(Mode::Radial(n)) = self.mode else {
            return false;
        };
        let Some((key, _)) = host.mod_key(MOD_ID, &key_id(n)) else {
            return false;
        };
        if key != input {
            return false;
        }
        self.pick(host, at, size, n);
        true
    }

    fn pick(&mut self, host: &dyn ClientHost, at: [f32; 2], size: [f32; 2], n: usize) {
        let entries = self.bundles(host).0[n].clone();
        let center = [size[0] / 2.0, size[1] / 2.0];
        self.fire = bundles::slice_at(center, at, entries.len()).map(|i| entries[i].clone());
        self.close = true;
    }

    fn editor(&mut self, size: [f32; 2]) -> &mut (Widgets, Ids, [f32; 2]) {
        if self.editor.as_ref().is_none_or(|e| e.2 != size) {
            let x = (size[0] / 2.0 - 155.0).floor();
            let y = size[1] - 26.0;
            let mut w = Widgets::new();
            let prev = w.add_button(Button::new([x, y, 76.0, 20.0], Text::literal("<")));
            let next = w.add_button(Button::new([x + 78.0, y, 76.0, 20.0], Text::literal(">")));
            let add = w.add_button(Button::new([x + 156.0, y, 76.0, 20.0], Text::literal("+")));
            let done = w.add_button(Button::new(
                [x + 234.0, y, 76.0, 20.0],
                Text::translate("gui.done", Vec::new()),
            ));
            self.editor = Some((
                w,
                Ids {
                    prev,
                    next,
                    add,
                    done,
                },
                size,
            ));
        }
        self.editor.as_mut().expect("built above")
    }
}

/// Draws one slice of the ring as quads along its arc.
fn draw_slice(
    draw: &mut dyn mod_api::DrawContext,
    center: [f32; 2],
    index: usize,
    count: usize,
    argb: u32,
) {
    let (a0, a1) = bundles::slice_angles(index, count);
    let gap = 0.02;
    let steps = 8;
    let point = |r: f32, a: f32| [center[0] + r * a.sin(), center[1] - r * a.cos()];
    for s in 0..steps {
        let t0 = a0 + gap + (a1 - a0 - 2.0 * gap) * s as f32 / steps as f32;
        let t1 = a0 + gap + (a1 - a0 - 2.0 * gap) * (s + 1) as f32 / steps as f32;
        draw.quad(
            [
                point(INNER, t0),
                point(OUTER, t0),
                point(OUTER, t1),
                point(INNER, t1),
            ],
            argb,
        );
    }
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

fn ours(host: &dyn ClientHost, id: &str) -> bool {
    id == "mod" && host.mod_screen() == Some(MOD_ID)
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for n in 0..BUNDLES {
            r.register_key_mapping(bundle_key(n));
        }
        r.register_key_mapping(
            KeyRegistration::new("edit", "key.keyboard.unknown")
                .label("key.keybindbundles.edit")
                .category("key.categories.keybindbundles"),
        );
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        // Bundles added in the editor, or saved from an earlier session, get their key now.
        let count = self.with(|s| s.bundles(&*host).0.len());
        let from = self.with(|s| s.registered.max(BUNDLES));
        for n in from..count {
            host.register_runtime_key_mapping(bundle_key(n));
        }
        self.with(|s| s.registered = count.max(BUNDLES));
        let mut pressed = None;
        for n in 0..count {
            if host.consume_click(MOD_ID, &key_id(n)) && pressed.is_none() {
                pressed = Some(n);
            }
        }
        let edit = host.consume_click(MOD_ID, "edit");
        let (open, close, fire) = self.with(|s| {
            if let Some(n) = pressed {
                let empty = s.bundles(&*host).0[n].is_empty();
                s.open = Some(if empty {
                    Mode::Editor(n)
                } else {
                    Mode::Radial(n)
                });
            } else if edit {
                s.open = Some(Mode::Editor(0));
            }
            (s.open.take(), std::mem::take(&mut s.close), s.fire.take())
        });
        if close {
            host.mod_screen_close(MOD_ID);
            self.with(|s| s.mode = None);
        }
        if let Some(id) = fire {
            // The screen is closed now, so the press is taken as the binding's own key press. A
            // binding no mod provides now is skipped.
            let _ = host.press_binding(&id);
        }
        if let Some(mode) = open
            && host.mod_screen_open(MOD_ID)
        {
            self.with(|s| s.mode = Some(mode));
        }
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        if !ours(host, event.screen.id) {
            return;
        }
        let size = event.draw.size();
        let mouse = event.screen.mouse;
        self.with(|s| match s.mode {
            Some(Mode::Radial(n)) => {
                let entries = s.bundles(host).0[n].clone();
                let names = names(host);
                let center = [size[0] / 2.0, size[1] / 2.0];
                let hover = bundles::slice_at(center, mouse, entries.len());
                let draw = &mut *event.draw;
                for (i, id) in entries.iter().enumerate() {
                    let color = if hover == Some(i) {
                        0xC0E0_E0E0
                    } else {
                        0x9020_2020
                    };
                    draw_slice(draw, center, i, entries.len(), color);
                    let (a0, a1) = bundles::slice_angles(i, entries.len());
                    let mid = (a0 + a1) / 2.0;
                    let r = (INNER + OUTER) / 2.0;
                    let name = names.get(id).cloned().unwrap_or_else(|| id.clone());
                    let w = draw.text_width(&name);
                    let at = [
                        center[0] + r * mid.sin() - w / 2.0,
                        center[1] - r * mid.cos() - 4.0,
                    ];
                    draw.text(&name, at, 0xFFFF_FFFF, true);
                }
                let title = tr(host, &format!("key.keybindbundles.bundle_{}", n + 1));
                let w = draw.text_width(&title);
                draw.text(
                    &title,
                    [center[0] - w / 2.0, center[1] - 4.0],
                    0xFFFF_FF80,
                    true,
                );
            }
            Some(Mode::Editor(n)) => {
                let entries = s.bundles(host).0[n].clone();
                let draw = &mut *event.draw;
                draw.fill([0.0, 0.0, size[0], size[1]], 0xC010_1010);
                let title = Text::translate(
                    "keybindbundles.editor",
                    vec![Text::translate(
                        format!("key.keybindbundles.bundle_{}", n + 1),
                        Vec::new(),
                    )],
                );
                let title = host.text_string(&title).unwrap_or_default();
                let tw = draw.text_width(&title);
                draw.text(&title, [(size[0] - tw) / 2.0, 8.0], 0xFFFF_FFFF, true);
                let left = (size[0] / 2.0 - 155.0).floor();
                let rows = editor_rows(host);
                let bottom = size[1] - 32.0;
                let max_scroll = (rows.len() as f32 * ROW - (bottom - LIST_TOP)).max(0.0);
                s.scroll = s.scroll.clamp(0.0, max_scroll);
                let _ = draw.push_clip([0.0, LIST_TOP, size[0], bottom - LIST_TOP]);
                for (i, (id, name)) in rows.iter().enumerate() {
                    let y = LIST_TOP + i as f32 * ROW - s.scroll;
                    if y + ROW < LIST_TOP || y > bottom {
                        continue;
                    }
                    if inside([left, y, 310.0, ROW], mouse) {
                        draw.fill([left - 2.0, y, 314.0, ROW], 0x40FF_FFFF);
                    }
                    let picked = entries.iter().position(|e| e == id);
                    let color = if picked.is_some() {
                        0xFFA0_FFA0
                    } else {
                        0xFFFF_FFFF
                    };
                    draw.text(name, [left, y + 3.0], color, false);
                    if let Some(p) = picked {
                        let mark = format!("#{}", p + 1);
                        let w = draw.text_width(&mark);
                        draw.text(&mark, [left + 310.0 - w, y + 3.0], color, false);
                    }
                }
                let _ = draw.pop_clip();
                let (w, _, _) = s.editor(size);
                let _ = w.draw(&mut *event.draw, host, mouse);
            }
            None => {}
        });
    }

    fn screen_key_released_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenKeyReleasedPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        let size = [
            event.screen.rect[0] + event.screen.rect[2],
            event.screen.rect[1] + event.screen.rect[3],
        ];
        if self.with(|s| s.release(host, event.key, event.screen.mouse, size)) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn screen_mouse_button_released_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonReleasedPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        let size = [
            event.screen.rect[0] + event.screen.rect[2],
            event.screen.rect[1] + event.screen.rect[3],
        ];
        let name = mouse_name(event.button);
        if self.with(|s| s.release(host, &name, event.screen.mouse, size)) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        let at = event.screen.mouse;
        let size = [
            event.screen.rect[0] + event.screen.rect[2],
            event.screen.rect[1] + event.screen.rect[3],
        ];
        self.with(|s| match s.mode {
            Some(Mode::Radial(n)) if event.button == 0 => {
                s.pick(host, at, size, n);
                Flow::Cancel
            }
            Some(Mode::Editor(n)) => {
                let (w, ids, _) = s.editor(size);
                let (prev, next, add, done) = (ids.prev, ids.next, ids.add, ids.done);
                let outcome = w.mouse_pressed(host, at, event.button, event.modifiers);
                match outcome {
                    Outcome::Pressed(id) if id == prev => {
                        let count = s.bundles(host).0.len();
                        s.mode = Some(Mode::Editor((n + count - 1) % count));
                        s.scroll = 0.0;
                    }
                    Outcome::Pressed(id) if id == next => {
                        let count = s.bundles(host).0.len();
                        s.mode = Some(Mode::Editor((n + 1) % count));
                        s.scroll = 0.0;
                    }
                    Outcome::Pressed(id) if id == add => {
                        // Its key is registered on the next client tick.
                        if s.bundles(host).add_bundle() {
                            s.save(host);
                        }
                    }
                    Outcome::Pressed(id) if id == done => s.close = true,
                    Outcome::Ignored if event.button == 0 => {
                        let left = (size[0] / 2.0 - 155.0).floor();
                        if at[0] >= left
                            && at[0] < left + 310.0
                            && at[1] >= LIST_TOP
                            && at[1] < size[1] - 32.0
                        {
                            let row = ((at[1] - LIST_TOP + s.scroll) / ROW) as usize;
                            if let Some((id, _)) = editor_rows(host).get(row) {
                                s.bundles(host).toggle(n, id);
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
            }
            _ => Flow::Continue,
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
        self.with(|s| {
            if matches!(s.mode, Some(Mode::Editor(_))) {
                s.scroll -= event.scroll[1] as f32 * ROW * 2.0;
                Flow::Cancel
            } else {
                Flow::Continue
            }
        })
    }
}
