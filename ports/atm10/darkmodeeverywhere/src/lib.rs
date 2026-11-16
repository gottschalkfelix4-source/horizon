//! Dark Mode Everywhere for Horizon: the game's and every mod's screens drawn dark.
//!
//! Written from Dark Mode Everywhere's observed behaviour and public description, never from its
//! code (docs/modpacks.md, "Licences"). The original swaps the GUI's shaders so light textures draw
//! dark. Horizon's mod API has no GUI colour filter yet (README, "Needs from Horizon"), so this port
//! lays a translucent black over a screen's panel once the panel is drawn and before its slots,
//! items, widgets and text, and redraws a chest's dark labels in a light colour. A button in the
//! bottom left corner cycles the darkness; Shift-click puts the screen on the blacklist or takes it
//! off. The model is in `dark.rs`.
use std::sync::Mutex;

use mod_api::{
    ClientHost, ConfigPath, Flow, Hooks, Registrar, RegistrationContext,
    ScreenMouseButtonPressedPreEvent, ScreenRenderBackgroundEvent, ScreenRenderForegroundEvent,
    ScreenRenderPostEvent, ScreenView, SettingSpec, Text,
};

pub mod dark;

use dark::{Dark, PRESETS};

/// This mod's id, as `horizon-mod.toml` names it.
pub const MOD_ID: &str = "darkmodeeverywhere";
const FILE: &str = "state.txt";
/// GLFW's Shift bit in the screen hooks' `modifiers`.
const SHIFT: u32 = 1;
/// The light colour a darkened container's labels are redrawn in.
const LABEL: u32 = 0xFFE0_E0E0;

fn button(size: [f32; 2]) -> [f32; 4] {
    [4.0, size[1] - 24.0, 20.0, 20.0]
}

fn inside(rect: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= rect[0] && at[0] < rect[0] + rect[2] && at[1] >= rect[1] && at[1] < rect[1] + rect[3]
}

/// The window's size: the panel-less screens' `rect` is the whole window; a container's is not,
/// but the window is at least as big as twice its offset plus itself (it is centred).
fn window(screen: &ScreenView<'_>) -> [f32; 2] {
    [
        screen.rect[0] * 2.0 + screen.rect[2],
        screen.rect[1] * 2.0 + screen.rect[3],
    ]
}

#[derive(Default)]
struct Mod {
    dark: Mutex<Option<Dark>>,
}

impl Mod {
    fn with<R>(&self, host: &dyn ClientHost, f: impl FnOnce(&mut Dark) -> R) -> R {
        let mut slot = self.dark.lock().unwrap_or_else(|e| e.into_inner());
        let dark = slot.get_or_insert_with(|| {
            let text = ConfigPath::new(FILE)
                .ok()
                .and_then(|p| host.data_files().read(&p).ok())
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default();
            Dark::parse(&text)
        });
        f(dark)
    }

    fn overlay(&self, host: &dyn ClientHost, screen: &ScreenView<'_>) -> Option<u32> {
        self.with(host, |d| {
            d.overlay(dark::screen_key(screen.id, screen.menu))
        })
    }
}

fn save(host: &dyn ClientHost, dark: &Dark) {
    if let Ok(p) = ConfigPath::new(FILE) {
        let _ = host.data_files().write(&p, dark.write().as_bytes());
    }
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let _ = r.register_setting(SettingSpec::toggle(
            "button",
            "darkmodeeverywhere.setting.button",
            true,
        ));
        let _ = r.register_setting(
            SettingSpec::toggle(
                "light_labels",
                "darkmodeeverywhere.setting.light_labels",
                true,
            )
            .tooltip("darkmodeeverywhere.setting.light_labels.tooltip"),
        );
    }

    /// The panel is drawn: darken it before the slots, items and widgets go on it.
    fn screen_render_background(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenRenderBackgroundEvent<'_>,
    ) {
        let size = event.draw.size();
        let rect = event.screen.rect;
        // A screen with no panel (the title, options, chat) has nothing light to darken: its
        // background is the panorama or the blurred world, and its buttons need a colour filter.
        if rect[2] >= size[0] && rect[3] >= size[1] {
            return;
        }
        if let Some(argb) = self.overlay(host, event.screen) {
            event.draw.fill(rect, argb);
        }
    }

    /// Redraw a darkened chest's labels in a light colour over vanilla's dark ones.
    fn screen_render_foreground(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenRenderForegroundEvent<'_>,
    ) {
        let screen = event.screen;
        let (Some(menu), Some(title)) = (screen.menu, screen.title) else {
            return;
        };
        if self.overlay(host, screen).is_none()
            || !host.setting_bool("light_labels").unwrap_or(true)
        {
            return;
        }
        let Some((title_at, inventory_at)) = dark::label_spots(menu, screen.rect[3]) else {
            return;
        };
        let [x, y, _, _] = screen.rect;
        if let Ok(title) = host.text_string(title) {
            event
                .draw
                .text(&title, [x + title_at[0], y + title_at[1]], LABEL, false);
        }
        let inventory = host
            .text_string(&Text::translate("container.inventory", Vec::new()))
            .unwrap_or_default();
        event.draw.text(
            &inventory,
            [x + inventory_at[0], y + inventory_at[1]],
            LABEL,
            false,
        );
    }

    /// The corner button, and its tooltip while hovered.
    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        if !host.setting_bool("button").unwrap_or(true) {
            return;
        }
        let rect = button(event.draw.size());
        let hovered = inside(rect, event.screen.mouse);
        let _ = event
            .draw
            .widget_button(rect, &Text::literal("\u{263e}"), true, hovered);
        if hovered {
            let key = dark::screen_key(event.screen.id, event.screen.menu).to_owned();
            let (preset, listed) =
                self.with(host, |d| (PRESETS[d.preset].0, d.blacklist.contains(&key)));
            let lines = [
                Text::translate(
                    "darkmodeeverywhere.button",
                    vec![Text::translate(preset, Vec::new())],
                ),
                Text::translate(
                    if listed {
                        "darkmodeeverywhere.button.unblacklist"
                    } else {
                        "darkmodeeverywhere.button.blacklist"
                    },
                    vec![Text::literal(key)],
                ),
            ];
            let _ = event.draw.gui_tooltip(&lines, event.screen.mouse);
        }
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        if event.button != 0 || !host.setting_bool("button").unwrap_or(true) {
            return Flow::Continue;
        }
        if !inside(button(window(event.screen)), event.screen.mouse) {
            return Flow::Continue;
        }
        let key = dark::screen_key(event.screen.id, event.screen.menu).to_owned();
        let shift = event.modifiers & SHIFT != 0;
        self.with(host, |d| {
            if shift {
                d.toggle_blacklist(&key);
            } else {
                d.cycle();
            }
            save(host, d);
        });
        Flow::Cancel
    }
}
