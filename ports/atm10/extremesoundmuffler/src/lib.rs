//! Extreme Sound Muffler for Horizon: turn down or mute any sound, everywhere or inside anchors.
//!
//! Written from Extreme Sound Muffler's observed behaviour and public description, never from its
//! code (docs/modpacks.md, "Licences"). Every sound the client is about to play goes through
//! `play_sound`, which notes it in the recent list and scales its volume by the player's choices
//! (`muffle.rs`). The screen, opened from its key or the note button over the inventory, lists the
//! sounds just heard, every sound, or the muffled ones; a click on a sound mutes it (at the
//! default muffle volume) or lets it play again, and `-` / `+` set its volume. "Target" picks where
//! the choice applies: everywhere, or one of ten anchors set at the player's position.
//!
//! Licence: LGPL-3.0-only, as the original (LICENSE-ORIGINAL.md).
use std::sync::Mutex;

use mod_api::client::widgets::{Button, Outcome, TextField, WidgetId, Widgets};
use mod_api::{
    ClientHost, ClientTickPostEvent, ConfigPath, Flow, Hooks, KeyRegistration, LoopContext,
    PlaySoundEvent, Registrar, RegistrationContext, ScreenCharTypedPreEvent,
    ScreenKeyPressedPreEvent, ScreenMouseButtonPressedPreEvent, ScreenMouseScrolledPreEvent,
    ScreenRenderPostEvent, SettingSpec, Text,
};

pub mod muffle;

use muffle::{Anchor, DEFAULT_RADIUS, MAX_RADIUS, Muffler, Place, Recent};

/// This mod's id, as `horizon-mod.toml` names it.
pub const MOD_ID: &str = "extremesoundmuffler";
const FILE: &str = "muffler.txt";
const ROW: f32 = 14.0;
const LIST_TOP: f32 = 94.0;

/// Which sounds the list shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum List {
    #[default]
    Recent,
    All,
    Muffled,
}

impl List {
    fn next(self) -> Self {
        match self {
            List::Recent => List::All,
            List::All => List::Muffled,
            List::Muffled => List::Recent,
        }
    }
    fn label(self) -> &'static str {
        match self {
            List::Recent => "extremesoundmuffler.list.recent",
            List::All => "extremesoundmuffler.list.all",
            List::Muffled => "extremesoundmuffler.list.muffled",
        }
    }
}

struct Ids {
    list: WidgetId,
    target: WidgetId,
    enabled: WidgetId,
    search: WidgetId,
    set_here: WidgetId,
    smaller: WidgetId,
    bigger: WidgetId,
    clear: WidgetId,
    done: WidgetId,
}

#[derive(Default)]
struct State {
    muffler: Option<Muffler>,
    recent: Recent,
    /// The dimension the player is in, for `play_sound`, which has no host.
    dimension: String,
    list: List,
    /// None for global, Some(n) for anchor n.
    target: Option<usize>,
    search: String,
    scroll: f32,
    widgets: Option<(Widgets, Ids, [f32; 2])>,
    open: bool,
    close: bool,
    /// Set the target anchor at the player, on the next tick (it reads the camera).
    place_anchor: bool,
    dirty: bool,
}

fn tr(host: &dyn ClientHost, key: &str) -> String {
    host.translate(key).unwrap_or_else(|| key.to_owned())
}

fn t(key: &str) -> Text {
    Text::translate(key, Vec::new())
}

fn inside(rect: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= rect[0] && at[0] < rect[0] + rect[2] && at[1] >= rect[1] && at[1] < rect[1] + rect[3]
}

/// The note button over the inventory panel's top right corner.
fn inventory_button(panel: [f32; 4]) -> [f32; 4] {
    [panel[0] + panel[2] - 20.0, panel[1] - 22.0, 20.0, 20.0]
}

fn is_inventory(screen: &mod_api::ScreenView<'_>) -> bool {
    (screen.id == "inventory" && screen.menu.is_none()) || screen.id == "creative"
}

impl State {
    fn muffler(&mut self, host: &dyn ClientHost) -> &mut Muffler {
        self.muffler.get_or_insert_with(|| {
            let text = ConfigPath::new(FILE)
                .ok()
                .and_then(|p| host.data_files().read(&p).ok())
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default();
            Muffler::parse(&text)
        })
    }

    fn save(&mut self, host: &dyn ClientHost) {
        let text = self.muffler(host).write();
        if let Ok(p) = ConfigPath::new(FILE) {
            let _ = host.data_files().write(&p, text.as_bytes());
        }
    }

    fn build(&mut self, size: [f32; 2]) {
        if self.widgets.as_ref().is_some_and(|w| w.2 == size) {
            return;
        }
        let x = (size[0] / 2.0 - 155.0).floor();
        let mut w = Widgets::new();
        let list = w.add_button(Button::new([x, 20.0, 100.0, 20.0], Text::literal("")));
        let target = w.add_button(Button::new(
            [x + 105.0, 20.0, 100.0, 20.0],
            Text::literal(""),
        ));
        let enabled = w.add_button(Button::new(
            [x + 210.0, 20.0, 100.0, 20.0],
            Text::literal(""),
        ));
        let mut field = TextField::new([x, 44.0, 310.0, 20.0], 64);
        field.set_value(&self.search);
        field.suggestion = None;
        let search = w.add_text_field(field);
        let set_here = w.add_button(Button::new(
            [x, 68.0, 100.0, 20.0],
            t("extremesoundmuffler.anchor.set"),
        ));
        let smaller = w.add_button(Button::new(
            [x + 105.0, 68.0, 48.0, 20.0],
            Text::literal("-"),
        ));
        let bigger = w.add_button(Button::new(
            [x + 157.0, 68.0, 48.0, 20.0],
            Text::literal("+"),
        ));
        let clear = w.add_button(Button::new(
            [x + 210.0, 68.0, 100.0, 20.0],
            t("extremesoundmuffler.anchor.clear"),
        ));
        let done = w.add_button(Button::new(
            [x + 105.0, size[1] - 26.0, 100.0, 20.0],
            t("gui.done"),
        ));
        self.widgets = Some((
            w,
            Ids {
                list,
                target,
                enabled,
                search,
                set_here,
                smaller,
                bigger,
                clear,
                done,
            },
            size,
        ));
    }

    /// Puts the current choices on the buttons, and greys out the anchor ones for "everywhere".
    fn relabel(&mut self, host: &dyn ClientHost) {
        let enabled = self.muffler(host).enabled;
        let anchor_set = self
            .target
            .is_some_and(|n| self.muffler(host).anchors[n].is_some());
        let target = match self.target {
            None => t("extremesoundmuffler.target.global"),
            Some(n) => Text::translate(
                "extremesoundmuffler.target.anchor",
                vec![Text::literal(n.to_string())],
            ),
        };
        let list = self.list;
        let Some((w, ids, _)) = &mut self.widgets else {
            return;
        };
        let set = |w: &mut Widgets, id, label: Option<Text>, active: bool| {
            if let Some(b) = w.button_mut(id) {
                if let Some(label) = label {
                    b.label = label;
                }
                b.active = active;
            }
        };
        set(
            w,
            ids.list,
            Some(Text::translate(
                "extremesoundmuffler.list",
                vec![t(list.label())],
            )),
            true,
        );
        set(w, ids.target, Some(target), true);
        set(
            w,
            ids.enabled,
            Some(t(if enabled {
                "extremesoundmuffler.enabled.on"
            } else {
                "extremesoundmuffler.enabled.off"
            })),
            true,
        );
        set(w, ids.set_here, None, self.target.is_some());
        set(w, ids.smaller, None, anchor_set);
        set(w, ids.bigger, None, anchor_set);
        set(w, ids.clear, None, anchor_set);
    }

    /// The sounds the list shows now.
    fn rows(&mut self, host: &dyn ClientHost) -> Vec<String> {
        let source: Vec<String> = match self.list {
            List::Recent => self.recent.0.iter().cloned().collect(),
            List::All => host
                .registry_entries("minecraft:sound_event")
                .map(|ids| {
                    let mut ids: Vec<String> = ids.iter().map(ToString::to_string).collect();
                    ids.sort();
                    ids
                })
                .unwrap_or_else(|| self.recent.0.iter().cloned().collect()),
            List::Muffled => {
                let target = self.target;
                self.muffler(host)
                    .volumes_mut(target)
                    .map(|v| v.keys().cloned().collect())
                    .unwrap_or_default()
            }
        };
        muffle::search(&source, &self.search)
            .into_iter()
            .cloned()
            .collect()
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

/// The list's columns: the volume's `-`, its text and its `+`, at the row's right end.
fn columns(left: f32) -> ([f32; 2], f32, [f32; 2]) {
    let right = left + 310.0;
    ([right - 60.0, 14.0], right - 30.0, [right - 14.0, 14.0])
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        r.register_key_mapping(
            KeyRegistration::new("open", "key.keyboard.unknown")
                .label("key.extremesoundmuffler.open")
                .category("key.categories.extremesoundmuffler"),
        );
        let _ = r.register_setting(
            SettingSpec::range(
                "default_volume",
                "extremesoundmuffler.setting.default_volume",
                0.0,
                0.9,
                0.1,
                0.0,
            )
            .tooltip("extremesoundmuffler.setting.default_volume.tooltip"),
        );
        let _ = r.register_setting(SettingSpec::toggle(
            "inventory_button",
            "extremesoundmuffler.setting.inventory_button",
            true,
        ));
    }

    /// Scales every sound by the player's choices, and notes it in the recent list.
    fn play_sound(&self, _: &dyn LoopContext, event: &mut PlaySoundEvent<'_>) {
        let Some(sound) = event.sound.as_mut() else {
            return;
        };
        let mut state = match self.state.try_lock() {
            Ok(s) => s,
            // The screen holds the state this instant: let this one sound play as it is.
            Err(_) => return,
        };
        if event.starting {
            state.recent.heard(&sound.identifier);
        }
        let Some(muffler) = &state.muffler else {
            return;
        };
        let place = Place {
            dimension: &state.dimension,
            pos: sound.position,
            relative: sound.relative,
        };
        let factor = muffler.factor(&sound.identifier, place);
        if factor <= 0.0 {
            *event.sound = None;
        } else if factor < 1.0 {
            sound.volume *= factor;
        }
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        let clicked = host.consume_click(MOD_ID, "open");
        let camera = host.camera().map(|c| c.position);
        let dimension = host.dimension().to_owned();
        let (open, close) = self.with(|s| {
            s.dimension = dimension.clone();
            // Load the choices before the first sound plays.
            s.muffler(&*host);
            if std::mem::take(&mut s.place_anchor)
                && let (Some(n), Some(pos)) = (s.target, camera)
            {
                let old = s.muffler(&*host).anchors[n].take();
                s.muffler(&*host).anchors[n] = Some(Anchor {
                    dimension: dimension.clone(),
                    pos: pos.map(f64::floor),
                    radius: old.as_ref().map_or(DEFAULT_RADIUS, |a| a.radius),
                    sounds: old.map(|a| a.sounds).unwrap_or_default(),
                });
                s.dirty = true;
            }
            if std::mem::take(&mut s.dirty) {
                s.save(&*host);
            }
            (
                std::mem::take(&mut s.open) || clicked,
                std::mem::take(&mut s.close),
            )
        });
        if close {
            host.mod_screen_close(MOD_ID);
        }
        if open {
            for _ in 0..4 {
                if host.mod_screen_open(MOD_ID) || !host.close_screen() {
                    break;
                }
            }
        }
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        if is_inventory(event.screen) && host.setting_bool("inventory_button").unwrap_or(true) {
            let rect = inventory_button(event.screen.rect);
            let _ = event.draw.widget_button(
                rect,
                &Text::literal("\u{266a}"),
                true,
                inside(rect, event.screen.mouse),
            );
            if inside(rect, event.screen.mouse) {
                let _ = event
                    .draw
                    .gui_tooltip(&[t("extremesoundmuffler.title")], event.screen.mouse);
            }
            return;
        }
        if !ours(host, event.screen.id) {
            return;
        }
        let size = event.draw.size();
        let mouse = event.screen.mouse;
        self.with(|s| {
            s.build(size);
            s.relabel(host);
            let rows = s.rows(host);
            let target = s.target;
            let draw = &mut *event.draw;
            draw.fill([0.0, 0.0, size[0], size[1]], 0xC010_1010);
            let title = tr(host, "extremesoundmuffler.title");
            let tw = draw.text_width(&title);
            draw.text(&title, [(size[0] - tw) / 2.0, 6.0], 0xFFFF_FFFF, true);
            let left = (size[0] / 2.0 - 155.0).floor();
            if let Some(n) = target {
                let info = match &s.muffler(host).anchors[n] {
                    Some(a) => Text::translate(
                        "extremesoundmuffler.anchor.info",
                        vec![
                            Text::literal(format!("{} {} {}", a.pos[0], a.pos[1], a.pos[2])),
                            Text::literal(a.radius.to_string()),
                        ],
                    ),
                    None => t("extremesoundmuffler.anchor.unset"),
                };
                let info = host.text_string(&info).unwrap_or_default();
                draw.text(&info, [left + 315.0, 74.0], 0xFFA0_A0A0, false);
            }
            let bottom = size[1] - 32.0;
            let max_scroll = (rows.len() as f32 * ROW - (bottom - LIST_TOP)).max(0.0);
            s.scroll = s.scroll.clamp(0.0, max_scroll);
            let (minus, pct, plus) = columns(left);
            let volumes = s
                .muffler(host)
                .volumes_mut(target)
                .cloned()
                .unwrap_or_default();
            let _ = draw.push_clip([0.0, LIST_TOP, size[0], bottom - LIST_TOP]);
            for (i, id) in rows.iter().enumerate() {
                let y = LIST_TOP + i as f32 * ROW - s.scroll;
                if y + ROW < LIST_TOP || y > bottom {
                    continue;
                }
                if inside([left, y, 310.0, ROW], mouse) {
                    draw.fill([left - 2.0, y, 314.0, ROW], 0x40FF_FFFF);
                }
                let muffled = volumes.get(id);
                let color = if muffled.is_some() {
                    0xFFFF_8080
                } else {
                    0xFFFF_FFFF
                };
                draw.text(id, [left, y + 3.0], color, false);
                if let Some(v) = muffled {
                    draw.text("-", [minus[0] + 4.0, y + 3.0], 0xFFFF_FFFF, false);
                    let p = format!("{}%", (v * 100.0).round());
                    let pw = draw.text_width(&p);
                    draw.text(&p, [pct - pw / 2.0, y + 3.0], 0xFFFF_FF80, false);
                    draw.text("+", [plus[0] + 4.0, y + 3.0], 0xFFFF_FFFF, false);
                }
            }
            let _ = draw.pop_clip();
            if let Some((w, _, _)) = &mut s.widgets {
                let _ = w.draw(draw, host, mouse);
            }
        });
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        let at = event.screen.mouse;
        if is_inventory(event.screen) && host.setting_bool("inventory_button").unwrap_or(true) {
            if event.button == 0 && inside(inventory_button(event.screen.rect), at) {
                self.with(|s| s.open = true);
                return Flow::Cancel;
            }
            return Flow::Continue;
        }
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        let default_volume = host.setting_f64("default_volume").unwrap_or(0.0) as f32;
        self.with(|s| {
            let Some((w, ids, size)) = &mut s.widgets else {
                return Flow::Continue;
            };
            let size = *size;
            let outcome = w.mouse_pressed(host, at, event.button, event.modifiers);
            let pressed = |id: WidgetId| outcome == Outcome::Pressed(id);
            let (list, target, enabled, set_here, smaller, bigger, clear, done) = (
                pressed(ids.list),
                pressed(ids.target),
                pressed(ids.enabled),
                pressed(ids.set_here),
                pressed(ids.smaller),
                pressed(ids.bigger),
                pressed(ids.clear),
                pressed(ids.done),
            );
            let radius = |s: &mut State, by: i32| {
                if let Some(n) = s.target
                    && let Some(a) = s.muffler(host).anchors[n].as_mut()
                {
                    a.radius = (a.radius as i32 + by).clamp(1, MAX_RADIUS as i32) as u32;
                    s.dirty = true;
                }
            };
            if list {
                s.list = s.list.next();
                s.scroll = 0.0;
            } else if target {
                s.target = match s.target {
                    None => Some(0),
                    Some(n) if n + 1 < muffle::ANCHORS => Some(n + 1),
                    Some(_) => None,
                };
            } else if enabled {
                let m = s.muffler(host);
                m.enabled = !m.enabled;
                s.dirty = true;
            } else if set_here {
                s.place_anchor = true;
            } else if smaller {
                radius(s, -4);
            } else if bigger {
                radius(s, 4);
            } else if clear {
                if let Some(n) = s.target {
                    s.muffler(host).anchors[n] = None;
                    s.dirty = true;
                }
            } else if done {
                s.close = true;
            } else if outcome == Outcome::Ignored && event.button == 0 {
                let left = (size[0] / 2.0 - 155.0).floor();
                let bottom = size[1] - 32.0;
                if inside([left, LIST_TOP, 310.0, bottom - LIST_TOP], at) {
                    let rows = s.rows(host);
                    let row = ((at[1] - LIST_TOP + s.scroll) / ROW) as usize;
                    if let Some(id) = rows.get(row) {
                        let (minus, _, plus) = columns(left);
                        let target = s.target;
                        let m = s.muffler(host);
                        let muffled = m.volumes_mut(target).is_some_and(|v| v.contains_key(id));
                        if muffled && at[0] >= minus[0] && at[0] < minus[0] + minus[1] {
                            m.nudge(target, id, -0.1);
                        } else if muffled && at[0] >= plus[0] && at[0] < plus[0] + plus[1] {
                            m.nudge(target, id, 0.1);
                        } else {
                            m.toggle(target, id, default_volume);
                        }
                        s.dirty = true;
                        return Flow::Cancel;
                    }
                }
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

    fn screen_key_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenKeyPressedPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        self.with(|s| {
            let Some((w, ids, _)) = &mut s.widgets else {
                return Flow::Continue;
            };
            let search = ids.search;
            let outcome = w.key_pressed(host, event.key, event.modifiers);
            if outcome == Outcome::Changed(search) {
                s.search = w
                    .text_field(search)
                    .map(|f| f.value().to_owned())
                    .unwrap_or_default();
                s.scroll = 0.0;
            }
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
            let Some((w, ids, _)) = &mut s.widgets else {
                return Flow::Continue;
            };
            let search = ids.search;
            let outcome = w.char_typed(host, event.c);
            if outcome == Outcome::Changed(search) {
                s.search = w
                    .text_field(search)
                    .map(|f| f.value().to_owned())
                    .unwrap_or_default();
                s.scroll = 0.0;
            }
            if outcome.handled() {
                Flow::Cancel
            } else {
                Flow::Continue
            }
        })
    }
}
