//! Toast Control for Horizon: settings that keep chosen kinds of toast from showing.
//!
//! Written from Toast Control's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and what a comparison with the NeoForge
//! original still has to confirm, is in README.md.
//!
//! A toast about to enter the queue (`toast_add`) is refused when its kind is blocked, before it
//! takes a slot, slides in or plays its sound.
use mod_api::{
    ClientHost, Flow, Hooks, Registrar, RegistrationContext, SettingSpec, Text, ToastAddEvent,
    ToastKind, text::TextContent,
};

pub const MOD_ID: &str = "toastcontrol";

/// The settings, by key; all off by default.
pub const SETTINGS: [&str; 7] = [
    "global_disable",
    "block_advancements",
    "block_challenges",
    "block_recipes",
    "block_tutorial",
    "block_system",
    "block_other",
];

/// Which kinds are blocked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Blocked {
    pub all: bool,
    pub advancements: bool,
    pub challenges: bool,
    pub recipes: bool,
    pub tutorial: bool,
    pub system: bool,
    pub other: bool,
}

impl Blocked {
    fn read(host: &dyn ClientHost) -> Self {
        let on = |key: &str| host.setting_bool(key).unwrap_or(false);
        Blocked {
            all: on("global_disable"),
            advancements: on("block_advancements"),
            challenges: on("block_challenges"),
            recipes: on("block_recipes"),
            tutorial: on("block_tutorial"),
            system: on("block_system"),
            other: on("block_other"),
        }
    }

    /// Whether a toast of `kind` titled `title` is kept out.
    pub fn blocks(&self, kind: ToastKind, title: &Text) -> bool {
        if self.all {
            return true;
        }
        match kind {
            ToastKind::Advancement => self.advancements || (self.challenges && is_challenge(title)),
            ToastKind::Recipe => self.recipes,
            ToastKind::Tutorial => self.tutorial,
            ToastKind::System => self.system,
            _ => self.other,
        }
    }
}

/// An advancement toast's title is its frame's label: `advancements.toast.challenge` for a
/// challenge.
fn is_challenge(title: &Text) -> bool {
    matches!(&title.content, TextContent::Translatable { key, .. } if key == "advancements.toast.challenge")
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for key in SETTINGS {
            let label = format!("{MOD_ID}.config.{key}");
            let _ = r.register_setting(
                SettingSpec::toggle(key, label.clone(), false).tooltip(format!("{label}.tooltip")),
            );
        }
    }

    fn toast_add(&self, host: &dyn ClientHost, event: &mut ToastAddEvent<'_>) -> Flow {
        if host.on(MOD_ID) && Blocked::read(host).blocks(event.kind, event.title) {
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(key: &str) -> Text {
        Text::translate(key, vec![])
    }

    #[test]
    fn nothing_is_blocked_by_default() {
        let none = Blocked::default();
        for kind in [
            ToastKind::Advancement,
            ToastKind::Recipe,
            ToastKind::Tutorial,
            ToastKind::System,
            ToastKind::Other,
        ] {
            assert!(!none.blocks(kind, &Text::literal("x")));
        }
    }

    #[test]
    fn blocking_recipes_leaves_advancements() {
        let b = Blocked {
            recipes: true,
            ..Blocked::default()
        };
        assert!(b.blocks(ToastKind::Recipe, &Text::literal("")));
        assert!(!b.blocks(ToastKind::Advancement, &frame("advancements.toast.task")));
    }

    #[test]
    fn challenges_can_be_blocked_alone() {
        let b = Blocked {
            challenges: true,
            ..Blocked::default()
        };
        assert!(b.blocks(
            ToastKind::Advancement,
            &frame("advancements.toast.challenge")
        ));
        assert!(!b.blocks(ToastKind::Advancement, &frame("advancements.toast.goal")));
    }

    #[test]
    fn global_disable_blocks_every_kind() {
        let b = Blocked {
            all: true,
            ..Blocked::default()
        };
        assert!(b.blocks(ToastKind::System, &Text::literal("")));
        assert!(b.blocks(ToastKind::Other, &Text::literal("")));
    }
}
