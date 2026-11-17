//! Every key binding as a mod sees it (`ClientHost::key_bindings`, `set_key_binding`,
//! `set_key_binding_hidden`, `press_binding`, `register_runtime_key_mapping`): vanilla's and
//! Horizon's mappings by name, a mod's as `<mod id>:<id>`.
use super::Game;
use horizon_client::keybinds;
use horizon_client::mod_list::KeyMapping as ListedMapping;
use mod_api::{
    ClientModifiers, KeyBindingView, KeyConflictContext, KeyRegistration, MappingError, Text,
};

/// The modifiers a mod's binding keeps: shift, control and alt.
const BOUND_MODIFIERS: u32 = 7;

impl Game<'_> {
    /// A mod's registration as Options > Controls lists it.
    pub(in crate::game) fn engine_key_mapping(k: &KeyRegistration) -> ListedMapping {
        ListedMapping {
            id: k.id.clone(),
            label: k.label.clone(),
            key: k.key.clone(),
            modifiers: k.modifiers.bits,
            context: crate::game::api_values::TryIntoEngine::try_into_engine(&k.context, &())
                .unwrap(),
            category: k.category.clone(),
        }
    }
    /// Every binding Controls lists: vanilla's and Horizon's, then each mod's.
    pub(in crate::game) fn key_bindings(&self) -> Vec<KeyBindingView> {
        let vanilla = keybinds::all_mappings().map(|&(name, default, category)| KeyBindingView {
            id: name.to_owned(),
            label: Text::translate(name, Vec::new()),
            category: format!("key.category.minecraft.{category}"),
            key: self.settings.keys.get(name).to_owned(),
            modifiers: ClientModifiers::default(),
            default_key: default.to_owned(),
            default_modifiers: ClientModifiers::default(),
            context: KeyConflictContext::Universal,
            owner: None,
        });
        let mods = self.declared_mod_mappings().into_iter().map(|(owner, k)| {
            let (key, modifiers) =
                self.settings
                    .mods
                    .binding(owner, &k.id, (&k.key, k.modifiers.bits));
            KeyBindingView {
                id: format!("{owner}:{}", k.id),
                label: Text::translate(k.label.clone(), Vec::new()),
                category: k.category.clone(),
                key,
                modifiers: ClientModifiers::new(modifiers),
                default_key: k.key.clone(),
                default_modifiers: k.modifiers,
                context: k.context,
                owner: Some(owner.to_owned()),
            }
        });
        vanilla.chain(mods).collect()
    }
    /// A mod's registered mapping `id` (`<mod id>:<id>`), as its owner and its own id.
    fn mod_mapping_id<'a>(&self, id: &'a str) -> Option<(&'a str, &'a str)> {
        let (owner, name) = id.split_once(':')?;
        self.declared_mod_mappings()
            .iter()
            .any(|(o, k)| *o == owner && k.id == name)
            .then_some((owner, name))
    }
    /// What changing a binding does that Controls does after a click: kept, bound, laid out again.
    fn key_bindings_changed(&mut self) {
        self.settings.save(&Self::options_path());
        self.settings.keys.sync_action_bindings();
        self.bind_mod_mappings();
        self.ui.options.layout_changed();
    }
    /// Rebinds `id` as Controls does: a vanilla or Horizon mapping to the key alone, a mod's with
    /// the modifiers held.
    pub(in crate::game) fn set_key_binding(
        &mut self,
        id: &str,
        key: &str,
        modifiers: ClientModifiers,
    ) -> Result<(), MappingError> {
        let unknown = |name: &str| MappingError::Unknown { key: name.into() };
        if key != "key.keyboard.unknown"
            && keybinds::key_code(key).is_none()
            && keybinds::mouse_button(key).is_none()
        {
            return Err(unknown(key));
        }
        if let Some(&(name, _, _)) = keybinds::all_mappings().find(|m| m.0 == id) {
            if !self.settings.extras.set_key(name, key.to_owned()) {
                self.settings.keys.set(name, key);
            }
        } else {
            let (owner, name) = self.mod_mapping_id(id).ok_or_else(|| unknown(id))?;
            self.settings
                .mods
                .set_binding(owner, name, key, modifiers.bits & BOUND_MODIFIERS);
        }
        self.key_bindings_changed();
        Ok(())
    }
    /// Leaves binding `id` off Controls, or lists it again.
    pub(in crate::game) fn set_key_binding_hidden(&mut self, id: &str, hidden: bool) {
        let hidden_keys = &mut self.settings.mods.hidden_keys;
        let changed = if hidden {
            hidden_keys.insert(id.to_owned())
        } else {
            hidden_keys.remove(id)
        };
        if changed {
            self.key_bindings_changed();
        }
    }
    /// One press and release of binding `id`, with no screen open.
    pub(in crate::game) fn press_binding(&mut self, id: &str) -> Result<(), MappingError> {
        if let Some(&(_, default, category)) = keybinds::all_mappings().find(|m| m.0 == id) {
            if matches!(category, "movement" | "gameplay" | "inventory") {
                return self.click_mapping(id);
            }
            let code = keybinds::key_code(default)
                .ok_or_else(|| MappingError::NotGameplay { key: id.into() })?;
            if self.player.is_none() {
                return Err(MappingError::NoWorld);
            }
            if !self.mod_mappings_screen_open() {
                if let Err(error) = self.key_pressed(code, code) {
                    log::warn!("press_binding {id}: {error:#}");
                }
            }
            return Ok(());
        }
        let (owner, name) = self
            .mod_mapping_id(id)
            .ok_or_else(|| MappingError::Unknown { key: id.into() })?;
        if self.player.is_none() {
            return Err(MappingError::NoWorld);
        }
        if !self.mod_mappings_screen_open() {
            self.native_input.mappings.click(owner, name);
        }
        Ok(())
    }
    /// A key the running mod registers after startup; a later one of its id replaces it.
    pub(in crate::game) fn register_runtime_key_mapping(&mut self, mapping: KeyRegistration) {
        let Some(owner) = mod_api::content::crash::current_native_mod() else {
            log::warn!("register_runtime_key_mapping {}: not called from a mod", mapping.id);
            return;
        };
        let startup = mod_api::__private::key_mappings()
            .iter()
            .any(|(o, k)| *o == owner && k.id == mapping.id);
        if mapping.id.is_empty() || mapping.id.contains(':') || startup {
            log::warn!("{owner}: key mapping id {:?} refused", mapping.id);
            return;
        }
        let listed = Self::engine_key_mapping(&mapping);
        let runtime = &mut self.native_input.runtime;
        match runtime.iter_mut().find(|(o, k)| o == owner && k.id == mapping.id) {
            Some((_, old)) => *old = mapping,
            None => runtime.push((owner.to_owned(), mapping)),
        }
        let page = self
            .settings
            .mods
            .installed
            .iter_mut()
            .find(|row| row.id == owner)
            .and_then(|row| row.native.as_mut());
        if let Some(page) = page {
            match page.mappings.iter_mut().find(|m| m.id == listed.id) {
                Some(old) => *old = listed,
                None => page.mappings.push(listed),
            }
        }
        self.bind_mod_mappings();
        self.ui.options.layout_changed();
    }
}
