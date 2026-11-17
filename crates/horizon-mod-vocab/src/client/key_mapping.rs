//! The keys a native mod registers on Options > Controls.

use crate::{ClientModifiers, Text};
/// The screens in which two mappings can conflict.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
#[derive(horizon_mod_api_macros::Wire)]
pub enum KeyConflictContext {
    /// Conflicts on every screen.
    #[default]
    Universal,
    /// Conflicts while playing.
    InGame,
    /// Conflicts in a GUI.
    Gui,
}
impl KeyConflictContext {
    /// Whether these screen contexts overlap.
    pub fn conflicts(self, other: Self) -> bool {
        self == Self::Universal || other == Self::Universal || self == other
    }
}
#[cfg(test)]
mod context_tests {
    use super::KeyConflictContext::*;
    #[test]
    fn key_context_case_order() {
        assert_eq!([Universal as u8, InGame as u8, Gui as u8], [0, 1, 2]);
        assert!(Universal.conflicts(Gui));
        assert!(!InGame.conflicts(Gui));
    }
}

/// A key a mod registers through `Registrar::register_key_mapping`: it is listed under the mod's name on
/// Options > Controls, and the player's binding and reset are kept by mod id in `horizon-mods.txt`.
/// Its default is `key` with `modifiers` held; shift, control and alt are bound, super is not.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[derive(horizon_mod_api_macros::Wire)]
pub struct KeyRegistration {
    /// The key's id, unique within its mod (`zoom`).
    pub id: String,
    /// The translation key for the label Controls shows.
    pub label: String,
    /// The default key (`key.keyboard.z`).
    pub key: String,
    /// The default modifiers, in GLFW's input-event encoding (shift 1, control 2, alt 4).
    pub modifiers: ClientModifiers,
    /// The category translation key within its mod's heading; empty for none.
    pub category: String,
    /// When this mapping can conflict with another binding.
    pub context: KeyConflictContext,
}

impl KeyRegistration {
    /// Sets the conflict context.
    pub fn context(mut self, context: KeyConflictContext) -> Self {
        self.context = context;
        self
    }
    /// Key `id` of the registering mod, bound to `key` with no modifiers and labelled `id`.
    pub fn new(id: &str, key: &str) -> Self {
        Self {
            id: id.to_owned(),
            label: id.to_owned(),
            key: key.to_owned(),
            modifiers: ClientModifiers::default(),
            category: String::new(),
            context: KeyConflictContext::Universal,
        }
    }

    /// Sets the label translation key Controls shows.
    pub fn label(mut self, label: &str) -> Self {
        self.label = label.to_owned();
        self
    }

    /// Sets the modifiers the default binding holds.
    pub fn modifiers(mut self, modifiers: ClientModifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    /// Sets the category translation key the binding is listed under.
    pub fn category(mut self, category: &str) -> Self {
        self.category = category.to_owned();
        self
    }
}

/// One key binding Controls lists (`ClientHost::key_bindings`): vanilla's, Horizon's and every
/// mod's, with the key it is bound to now and the one it resets to.
#[derive(Clone, Debug, PartialEq)]
// exhaustive: the game builds it with a literal; mods only read it
#[derive(horizon_mod_api_macros::Wire)]
pub struct KeyBindingView {
    /// What `set_key_binding` and `press_binding` take: the mapping's name for vanilla's and
    /// Horizon's (`key.jump`, `key.emote`), `<mod id>:<id>` for a mod's (`zoom:in`).
    pub id: String,
    /// The label Controls shows, a translation key.
    pub label: Text,
    /// The category's translation key (`key.category.minecraft.movement`); a mod's own, or
    /// empty for none.
    pub category: String,
    /// The key or mouse button it is bound to (`key.keyboard.z`); `key.keyboard.unknown` for none.
    pub key: String,
    /// The modifiers held with it, in GLFW's input-event encoding (shift 1, control 2, alt 4).
    /// Only a mod's binding holds any.
    pub modifiers: ClientModifiers,
    /// The key Reset puts it back on.
    pub default_key: String,
    /// The modifiers Reset puts back.
    pub default_modifiers: ClientModifiers,
    /// When it can conflict with another binding; vanilla's and Horizon's are `Universal`.
    pub context: KeyConflictContext,
    /// The id of the mod that registered it; `None` for vanilla's and Horizon's.
    pub owner: Option<String>,
}
