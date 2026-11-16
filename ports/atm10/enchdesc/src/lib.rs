// SPDX-License-Identifier: LGPL-2.1-only
//! Enchantment Descriptions for Horizon: under each enchantment in an item's tooltip, a grey
//! line saying what it does (the text at `enchantment.<namespace>.<name>.desc`), by default only
//! while Shift is held, with a hint line otherwise.
//!
//! Written from Enchantment Descriptions' observed behaviour and public description, never from
//! its code (docs/modpacks.md, "Licences"); the description texts are this port's own. Released,
//! as the original, under LGPL-2.1-only. What a comparison with the NeoForge original still has
//! to confirm is in README.md.
use mod_api::{
    ClientHost, Hooks, ItemStack, ItemTooltipEvent, Registrar, RegistrationContext, SettingSpec,
    Tag, Text,
    text::{TextContent, TextStyle},
};

pub const MOD_ID: &str = "enchdesc";

/// The enchantment ids in an enchantments component: `{id: level}` (1.21.5 and later) or the
/// older `{levels: {id: level}}`.
pub fn enchantment_ids(tag: &Tag) -> Vec<String> {
    let Tag::Compound(c) = tag else {
        return Vec::new();
    };
    let levels = match c.get(&"levels".into()) {
        Some(Tag::Compound(levels)) => levels,
        _ => c,
    };
    levels.keys().map(|k| k.to_string_lossy()).collect()
}

/// The translation key of enchantment `id`'s name (`minecraft:sharpness` →
/// `enchantment.minecraft.sharpness`), as vanilla's `Util.makeDescriptionId`.
pub fn name_key(id: &str) -> String {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    format!("enchantment.{ns}.{}", path.replace('/', "."))
}

/// What to add to `lines`: `(index to insert at, line)`, in increasing order of index as the
/// lines stand before any insertion. With `show`, a description after each enchantment line
/// of `ids` that has one (`has_desc`); otherwise one hint after the last such line.
pub fn plan(
    lines: &[(Text, u32)],
    ids: &[String],
    has_desc: impl Fn(&str) -> bool,
    show: bool,
) -> Vec<(usize, Text)> {
    let keys: Vec<String> = ids.iter().map(|id| name_key(id)).collect();
    let mut gray = TextStyle::default();
    gray.color = Some("gray".into());
    let mut out = Vec::new();
    let mut last = None;
    for (i, (text, _)) in lines.iter().enumerate() {
        let TextContent::Translatable { key, .. } = &text.content else {
            continue;
        };
        if !keys.contains(key) {
            continue;
        }
        let desc = format!("{key}.desc");
        if !has_desc(&desc) {
            continue;
        }
        last = Some(i);
        if show {
            out.push((
                i + 1,
                Text::translate(desc, vec![]).with_style(gray.clone()),
            ));
        }
    }
    if !show && let Some(i) = last {
        let shift = Text::translate("key.keyboard.left.shift", vec![]);
        out.push((
            i + 1,
            Text::translate("enchdesc.activate.message", vec![shift]).with_style(gray),
        ));
    }
    out
}

/// A component of `stack`: its own patch first (a `!id` removal hides it), else its item's default.
fn component(host: &dyn ClientHost, stack: &ItemStack, id: &str) -> Option<Tag> {
    if let Some(patch) = stack.components() {
        let patch = patch.as_nbt();
        if patch.contains_key(&format!("!{id}").as_str().into()) {
            return None;
        }
        if let Some(tag) = patch.get(&id.into()) {
            return Some(tag.clone());
        }
    }
    host.item_default_component(stack.item(), id)
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for (key, default) in [("require_shift", true), ("only_on_books", false)] {
            let label = format!("{MOD_ID}.config.{key}");
            let _ = r.register_setting(
                SettingSpec::toggle(key, label.clone(), default)
                    .tooltip(format!("{label}.tooltip")),
            );
        }
    }

    fn item_tooltip(&self, host: &dyn ClientHost, event: &mut ItemTooltipEvent<'_>) {
        if !host.on(MOD_ID) {
            return;
        }
        let stored = component(host, event.stack, "minecraft:stored_enchantments");
        let only_books = host.setting_bool("only_on_books").unwrap_or(false);
        let mut ids = stored.as_ref().map(enchantment_ids).unwrap_or_default();
        if !only_books && let Some(tag) = component(host, event.stack, "minecraft:enchantments") {
            ids.extend(enchantment_ids(&tag));
        }
        if ids.is_empty() {
            return;
        }
        let show = !host.setting_bool("require_shift").unwrap_or(true) || host.modifiers().shift();
        let adds = plan(event.lines, &ids, |key| host.translate(key).is_some(), show);
        for (i, line) in adds.into_iter().rev() {
            event.lines.insert(i, (line, u32::MAX));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(key: &str) -> (Text, u32) {
        (Text::translate(key, vec![]), u32::MAX)
    }

    fn key_of(t: &Text) -> &str {
        match &t.content {
            TextContent::Translatable { key, .. } => key,
            _ => "",
        }
    }

    #[test]
    fn both_component_forms_name_their_enchantments() {
        let new = Tag::Compound([("minecraft:sharpness".into(), Tag::Int(5))].into());
        let old = Tag::Compound(
            [(
                "levels".into(),
                Tag::Compound([("minecraft:mending".into(), Tag::Int(1))].into()),
            )]
            .into(),
        );
        assert_eq!(enchantment_ids(&new), ["minecraft:sharpness"]);
        assert_eq!(enchantment_ids(&old), ["minecraft:mending"]);
        assert_eq!(
            name_key("mymod:fire/aspect"),
            "enchantment.mymod.fire.aspect"
        );
    }

    #[test]
    fn each_enchantment_gets_its_description_below_it() {
        let lines = [
            named("item.minecraft.diamond_sword"),
            named("enchantment.minecraft.sharpness"),
            named("enchantment.minecraft.unbreaking"),
            named("item.modifiers.mainhand"),
        ];
        let ids = [
            "minecraft:sharpness".to_string(),
            "minecraft:unbreaking".to_string(),
        ];
        let adds = plan(&lines, &ids, |_| true, true);
        let at: Vec<_> = adds.iter().map(|(i, t)| (*i, key_of(t))).collect();
        assert_eq!(
            at,
            [
                (2, "enchantment.minecraft.sharpness.desc"),
                (3, "enchantment.minecraft.unbreaking.desc")
            ]
        );
    }

    #[test]
    fn without_shift_one_hint_follows_the_last_enchantment() {
        let lines = [
            named("enchantment.minecraft.sharpness"),
            named("enchantment.minecraft.unbreaking"),
            named("item.modifiers.mainhand"),
        ];
        let ids = [
            "minecraft:sharpness".to_string(),
            "minecraft:unbreaking".to_string(),
        ];
        let adds = plan(&lines, &ids, |_| true, false);
        assert_eq!(adds.len(), 1);
        assert_eq!(
            (adds[0].0, key_of(&adds[0].1)),
            (2, "enchdesc.activate.message")
        );
    }

    #[test]
    fn an_enchantment_without_a_description_adds_nothing() {
        let lines = [named("enchantment.othermod.mystery")];
        let ids = ["othermod:mystery".to_string()];
        assert!(plan(&lines, &ids, |_| false, true).is_empty());
        assert!(plan(&lines, &ids, |_| false, false).is_empty());
    }
}
