//! Artifacts for Horizon, first slice: seven wearable artifacts and their effects.
//!
//! Written from the mod's public description and from how the artifacts play, never from its code
//! (docs/modpacks.md, "Licences"). Every number below is from memory and listed in README.md under
//! "To check against the NeoForge original".
//!
//! Horizon has no accessory slots (Curios or Accessories) yet, so an artifact counts as worn in
//! the vanilla slot nearest its own: a head artifact on the head, a necklace on the chest, shoes on
//! the feet, a glove in the off hand. Its data file makes it equippable there.
//!
//! How it works: `ItemPart::inventory_tick` sees each artifact in its slot every tick and records
//! it as worn by that player. At the end of the server tick each player's wanted attribute
//! modifiers are compared with the ones applied ([`reconcile`]): new ones are added (transient,
//! like a worn item's), gone ones removed. Effects are topped up once a second. The Cross Necklace
//! lengthens the invulnerability after a hit in `living_damage_pre`.
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use mod_api::{
    ActiveEffect, AttributeModifier, AttributeOperation, EffectKey, Identifier, InventoryTickEvent,
    ItemPart, LevelRandomHost, LivingDamagePreEvent, ModifierLifetime, Registrar,
    RegistrationContext, ServerHost, ServerTickEvent, ServerTickPhase,
};

/// The slots an artifact can be worn in, as `Inventory` numbers them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wear {
    Head,
    Necklace,
    Feet,
    Hand,
}

impl Wear {
    /// Whether an artifact worn this way counts in inventory slot `slot`.
    pub fn counts_in(self, slot: i32) -> bool {
        slot == match self {
            Wear::Head => 103,
            Wear::Necklace => 102,
            Wear::Feet => 100,
            Wear::Hand => -106,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Artifact {
    NightVisionGoggles,
    Snorkel,
    CrossNecklace,
    RunningShoes,
    BunnyHoppers,
    PowerGlove,
    FeralClaws,
}

/// An attribute change an artifact makes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Change {
    pub attribute: &'static str,
    pub amount: f64,
    pub operation: AttributeOperation,
}

const fn change(attribute: &'static str, amount: f64, operation: AttributeOperation) -> Change {
    Change {
        attribute,
        amount,
        operation,
    }
}

use AttributeOperation::{AddMultipliedTotal, AddValue};

const RUNNING_SHOES: &[Change] = &[
    change("minecraft:movement_speed", 0.4, AddMultipliedTotal),
    change("minecraft:step_height", 0.5, AddValue),
];
const BUNNY_HOPPERS: &[Change] = &[
    change("minecraft:jump_strength", 0.2, AddValue),
    change("minecraft:fall_damage_multiplier", -1.0, AddMultipliedTotal),
];
const POWER_GLOVE: &[Change] = &[change("minecraft:attack_damage", 4.0, AddValue)];
const FERAL_CLAWS: &[Change] = &[change("minecraft:attack_speed", 1.0, AddValue)];

impl Artifact {
    pub const ALL: [Artifact; 7] = [
        Artifact::NightVisionGoggles,
        Artifact::Snorkel,
        Artifact::CrossNecklace,
        Artifact::RunningShoes,
        Artifact::BunnyHoppers,
        Artifact::PowerGlove,
        Artifact::FeralClaws,
    ];

    pub fn path(self) -> &'static str {
        match self {
            Artifact::NightVisionGoggles => "night_vision_goggles",
            Artifact::Snorkel => "snorkel",
            Artifact::CrossNecklace => "cross_necklace",
            Artifact::RunningShoes => "running_shoes",
            Artifact::BunnyHoppers => "bunny_hoppers",
            Artifact::PowerGlove => "power_glove",
            Artifact::FeralClaws => "feral_claws",
        }
    }

    pub fn wear(self) -> Wear {
        match self {
            Artifact::NightVisionGoggles | Artifact::Snorkel => Wear::Head,
            Artifact::CrossNecklace => Wear::Necklace,
            Artifact::RunningShoes | Artifact::BunnyHoppers => Wear::Feet,
            Artifact::PowerGlove | Artifact::FeralClaws => Wear::Hand,
        }
    }

    /// The attribute changes the artifact makes on a wearer who is, or isn't, sprinting.
    pub fn changes(self, sprinting: bool) -> &'static [Change] {
        match self {
            Artifact::RunningShoes if sprinting => RUNNING_SHOES,
            Artifact::BunnyHoppers => BUNNY_HOPPERS,
            Artifact::PowerGlove => POWER_GLOVE,
            Artifact::FeralClaws => FERAL_CLAWS,
            _ => &[],
        }
    }

    /// The effect the artifact keeps on its wearer.
    pub fn effect(self) -> Option<&'static str> {
        match self {
            Artifact::NightVisionGoggles => Some("minecraft:night_vision"),
            Artifact::Snorkel => Some("minecraft:water_breathing"),
            _ => None,
        }
    }

    /// The modifier id of the artifact's change to `attribute`.
    pub fn modifier_id(self, attribute: &str) -> String {
        let attribute = attribute.rsplit(':').next().unwrap_or(attribute);
        format!("artifacts:{}_{attribute}", self.path())
    }
}

/// Ticks of invulnerability after a hit for a Cross Necklace wearer (vanilla gives 20).
pub const CROSS_NECKLACE_INVULNERABILITY: i32 = 40;
/// How long a topped-up effect lasts: past the night vision flicker (under 200 ticks).
pub const EFFECT_TICKS: i32 = 260;
/// How often effects are topped up.
pub const EFFECT_PERIOD: u64 = 20;

/// A modifier as one wearer wants it: (attribute, modifier id) is unique per entity.
pub type Wanted = (&'static str, String);

/// What to change so the applied modifiers become the wanted ones: (add, remove).
pub fn reconcile(
    applied: &BTreeSet<Wanted>,
    wanted: &BTreeSet<Wanted>,
) -> (Vec<Wanted>, Vec<Wanted>) {
    let add = wanted.difference(applied).cloned().collect();
    let remove = applied.difference(wanted).cloned().collect();
    (add, remove)
}

/// The modifiers a wearer of `worn` wants.
pub fn wanted(worn: &BTreeSet<Artifact>, sprinting: bool) -> BTreeSet<Wanted> {
    worn.iter()
        .flat_map(|a| {
            a.changes(sprinting)
                .iter()
                .map(move |c| (c.attribute, a.modifier_id(c.attribute)))
        })
        .collect()
}

fn change_of(artifact_and: &Wanted) -> Option<(Artifact, Change)> {
    Artifact::ALL.into_iter().find_map(|a| {
        [false, true].into_iter().find_map(|sprinting| {
            a.changes(sprinting)
                .iter()
                .find(|c| {
                    c.attribute == artifact_and.0 && a.modifier_id(c.attribute) == artifact_and.1
                })
                .map(|c| (a, *c))
        })
    })
}

#[derive(Default)]
struct Wearer {
    /// Artifacts seen worn during the current tick.
    worn: BTreeSet<Artifact>,
    applied: BTreeSet<Wanted>,
}

#[derive(Default)]
struct State {
    tick: u64,
    wearers: HashMap<u32, Wearer>,
    /// Players wearing a Cross Necklace, read by the damage hook.
    cross: HashSet<i32>,
}

fn state() -> std::sync::MutexGuard<'static, State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

static EFFECTS: OnceLock<HashMap<&'static str, EffectKey>> = OnceLock::new();

/// One artifact's item.
struct Worn(Artifact);

#[mod_api::horizon_part]
impl ItemPart for Worn {
    fn inventory_tick(&self, host: &mut dyn ServerHost, event: &mut InventoryTickEvent<'_>) {
        if !self.0.wear().counts_in(event.slot) {
            return;
        }
        let Some(player) = host.players().acting_player() else {
            return;
        };
        let id = player.entity().id();
        state().wearers.entry(id).or_default().worn.insert(self.0);
    }
}

fn end_of_tick(host: &mut dyn ServerHost) {
    let mut state = state();
    state.tick += 1;
    let tick = state.tick;
    let mut cross = HashSet::new();
    let mut gone = Vec::new();
    for (&id, wearer) in state.wearers.iter_mut() {
        let worn = std::mem::take(&mut wearer.worn);
        let Some(entity) = host.entity(id) else {
            gone.push(id);
            continue;
        };
        let Some(living) = host.living(entity) else {
            gone.push(id);
            continue;
        };
        let sprinting = entity.is_sprinting(host).unwrap_or(false);
        let (add, remove) = reconcile(&wearer.applied, &wanted(&worn, sprinting));
        for key in remove {
            if let Ok(modifier) = Identifier::parse(&key.1) {
                living.remove_attribute_modifier(host, key.0, &modifier);
            }
            wearer.applied.remove(&key);
        }
        for key in add {
            let (Some((_, c)), Ok(modifier)) = (change_of(&key), Identifier::parse(&key.1)) else {
                continue;
            };
            let modifier = AttributeModifier::new(modifier, c.amount, c.operation);
            if living.add_attribute_modifier(
                host,
                c.attribute,
                modifier,
                ModifierLifetime::Transient,
            ) {
                wearer.applied.insert(key);
            }
        }
        if tick % EFFECT_PERIOD == 0 {
            for effect in worn.iter().filter_map(|a| a.effect()) {
                if let Some(&key) = EFFECTS.get().and_then(|e| e.get(effect)) {
                    let effect = ActiveEffect::new(key, EFFECT_TICKS, 0, true, false, true);
                    living.add_effect(host, &effect);
                }
            }
        }
        if worn.contains(&Artifact::CrossNecklace) {
            cross.insert(id as i32);
        }
        if worn.is_empty() && wearer.applied.is_empty() {
            gone.push(id);
        }
    }
    for id in gone {
        state.wearers.remove(&id);
    }
    state.cross = cross;
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl mod_api::Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        for artifact in Artifact::ALL {
            let key = r.item_key(&format!("artifacts:{}", artifact.path()));
            r.item_part(key, Worn(artifact));
        }
        let effects = Artifact::ALL
            .into_iter()
            .filter_map(Artifact::effect)
            .map(|id| (id, r.effect_key(id)))
            .collect();
        let _ = EFFECTS.set(effects);
    }

    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        if matches!(event.phase, ServerTickPhase::ServerTickEnd { .. }) {
            end_of_tick(host);
        }
    }

    /// The Cross Necklace: a longer invulnerability after each hit.
    fn living_damage_pre(&self, _: &mut dyn LevelRandomHost, event: &mut LivingDamagePreEvent<'_>) {
        if event.kind == "minecraft:player" && state().cross.contains(&event.entity) {
            let ticks = event
                .container
                .post_attack_invulnerability_ticks()
                .max(CROSS_NECKLACE_INVULNERABILITY);
            event.container.set_post_attack_invulnerability_ticks(ticks);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifacts_count_only_in_their_own_slot() {
        assert!(Artifact::Snorkel.wear().counts_in(103));
        assert!(!Artifact::Snorkel.wear().counts_in(0));
        assert!(Artifact::RunningShoes.wear().counts_in(100));
        assert!(Artifact::PowerGlove.wear().counts_in(-106));
        assert!(!Artifact::PowerGlove.wear().counts_in(8));
        assert!(Artifact::CrossNecklace.wear().counts_in(102));
    }

    #[test]
    fn running_shoes_only_boost_a_sprinting_wearer() {
        let worn: BTreeSet<_> = [Artifact::RunningShoes].into();
        assert!(wanted(&worn, false).is_empty());
        let sprinting = wanted(&worn, true);
        assert!(sprinting.contains(&(
            "minecraft:movement_speed",
            "artifacts:running_shoes_movement_speed".to_string()
        )));
        assert_eq!(sprinting.len(), 2);
    }

    #[test]
    fn taking_an_artifact_off_removes_its_modifiers_and_keeps_the_rest() {
        let both: BTreeSet<_> = [Artifact::PowerGlove, Artifact::BunnyHoppers].into();
        let glove: BTreeSet<_> = [Artifact::PowerGlove].into();
        let applied = wanted(&both, false);
        let (add, remove) = reconcile(&applied, &wanted(&glove, false));
        assert!(add.is_empty());
        let mut removed: Vec<_> = remove.into_iter().map(|w| w.1).collect();
        removed.sort();
        assert_eq!(
            removed,
            [
                "artifacts:bunny_hoppers_fall_damage_multiplier",
                "artifacts:bunny_hoppers_jump_strength"
            ]
        );
    }

    #[test]
    fn a_wanted_modifier_maps_back_to_its_change() {
        let key = (
            "minecraft:attack_damage",
            "artifacts:power_glove_attack_damage".to_string(),
        );
        let (artifact, c) = change_of(&key).unwrap();
        assert_eq!(artifact, Artifact::PowerGlove);
        assert_eq!(c.amount, 4.0);
    }
}
