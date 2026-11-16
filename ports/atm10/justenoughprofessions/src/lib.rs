//! Just Enough Professions for Horizon: a JEI category with one recipe per villager profession,
//! showing its workstation block, a villager of that profession, and the profession's name.
//! `U` on a workstation shows which profession it gives.
//!
//! Written from the mod's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). README.md says what still has to be checked against it.
use std::sync::Arc;

use jei::{Category, Display, JeiPlugin, Registration, Role};
use mod_api::{Compound, Hooks, Host, Registrar, RegistrationContext, Tag};

pub const CATEGORY: &str = "justenoughprofessions:professions";

/// Vanilla's professions and the block each takes as its job site (its point of interest).
pub const PROFESSIONS: &[(&str, &str)] = &[
    ("armorer", "minecraft:blast_furnace"),
    ("butcher", "minecraft:smoker"),
    ("cartographer", "minecraft:cartography_table"),
    ("cleric", "minecraft:brewing_stand"),
    ("farmer", "minecraft:composter"),
    ("fisherman", "minecraft:barrel"),
    ("fletcher", "minecraft:fletching_table"),
    ("leatherworker", "minecraft:cauldron"),
    ("librarian", "minecraft:lectern"),
    ("mason", "minecraft:stonecutter"),
    ("shepherd", "minecraft:loom"),
    ("toolsmith", "minecraft:smithing_table"),
    ("weaponsmith", "minecraft:grindstone"),
];

pub const WIDTH: u16 = 120;
pub const HEIGHT: u16 = 50;

/// A villager's saved fields with `profession` (`minecraft:farmer`), a plains villager at its
/// first level, as `/summon minecraft:villager ~ ~ ~ {...}` reads them.
pub fn villager_data(profession: &str) -> Compound {
    let data: Compound = [
        ("profession".into(), Tag::String(profession.into())),
        ("type".into(), Tag::String("minecraft:plains".into())),
        ("level".into(), Tag::Int(1)),
    ]
    .into();
    [("VillagerData".into(), Tag::Compound(data))].into()
}

/// One profession's recipe: its workstation, an arrow, the villager, and its name under them
/// (vanilla's `entity.minecraft.villager.<profession>`).
pub fn display(namespace: &str, profession: &str, workstation: &str) -> Display {
    let id = format!("{namespace}:{profession}");
    Display::new(None)
        .slot(Role::Input, [6, 16], vec![workstation.to_owned()])
        .arrow([30, 16])
        .entity(
            "minecraft:villager",
            villager_data(&id),
            [62, 0, 40, 40],
            18.0,
        )
        .line(
            [0, 41],
            &format!("entity.{namespace}.villager.{profession}"),
            Vec::new(),
        )
}

/// The plugin JEI asks.
pub struct ProfessionsPlugin;

impl JeiPlugin for ProfessionsPlugin {
    fn uid(&self) -> &str {
        "justenoughprofessions:jei"
    }
    fn register_categories(&self, _: &dyn Host, r: &mut Registration) {
        r.add_category(Category::new(
            CATEGORY,
            "gui.justenoughprofessions.category.professions",
            "minecraft:emerald",
            WIDTH,
            HEIGHT,
        ));
    }
    fn register_recipes(&self, _: &dyn Host, r: &mut Registration) {
        for (profession, workstation) in PROFESSIONS {
            r.add_recipe(CATEGORY, display("minecraft", profession, workstation));
        }
    }
    fn register_catalysts(&self, _: &dyn Host, r: &mut Registration) {
        r.add_catalyst(CATEGORY, "minecraft:villager_spawn_egg");
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, _: &mut dyn Registrar, _: &RegistrationContext) {
        jei::register_plugin(Arc::new(ProfessionsPlugin));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jei::index::{Mode, RecipeIndex};

    #[test]
    fn a_lectern_makes_a_librarian() {
        let mut r = Registration::default();
        r.add_category(Category::new(
            CATEGORY,
            "k",
            "minecraft:emerald",
            WIDTH,
            HEIGHT,
        ));
        for (p, w) in PROFESSIONS {
            r.add_recipe(CATEGORY, display("minecraft", p, w));
        }
        r.add_catalyst(CATEGORY, "minecraft:villager_spawn_egg");
        let index = RecipeIndex::build(r);
        let found = index.find("minecraft:lectern", Mode::Uses);
        assert_eq!(found.len(), 1);
        let d = index.recipe(found[0].0, found[0].1[0]).unwrap();
        assert_eq!(d.lines[0].key, "entity.minecraft.villager.librarian");
        // The spawn egg's uses are every profession.
        assert_eq!(
            index.find("minecraft:villager_spawn_egg", Mode::Uses)[0]
                .1
                .len(),
            13
        );
    }

    #[test]
    fn the_villager_drawn_has_the_profession() {
        let data = villager_data("minecraft:farmer");
        let Some(Tag::Compound(v)) = data.get(&"VillagerData".into()) else {
            panic!("no villager data");
        };
        assert_eq!(
            v.get(&"profession".into()),
            Some(&Tag::String("minecraft:farmer".into()))
        );
    }
}
