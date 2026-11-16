//! Apotheosis for Horizon, first slice: affix loot. A monster a player kills may drop a piece of
//! gear of a rolled rarity (common to mythic) carrying rolled attribute affixes, named after its
//! rarity and coloured by it.
//!
//! Written from Apotheosis's observed behaviour and public description, never from its code or
//! data (docs/modpacks.md, "Licences"). Every number here (drop chance, rarity weights, affix
//! counts and value ranges) is from memory and has to be checked against the original: see
//! README.md.
//!
//! How it works: `living_drops` rolls the drop from the level's own random stream, so the draws
//! move vanilla's later ones as a Java mod's do. The stack records what was rolled, as Apotheosis
//! does, as a rarity and one *level* (0 to 1) per affix, in `minecraft:custom_data` under
//! `apotheosis`. The modifiers themselves are not written into the stack: the
//! `item_attribute_modifier` hook adds them every time the game reads the stack's modifiers,
//! computing each value from the affix, the rarity and the level. Vanilla's own modifiers (a
//! sword's base damage, a chestplate's armour) stay as they are.
use std::sync::OnceLock;

use mod_api::{
    AttributeModifier, AttributeOperation, Compound, EquipmentSlotGroup, Flow, Hooks, Identifier,
    ItemAttributeModifierEvent, ItemDrop, ItemKey, ItemStack, LivingDropsEvent, MobCategory,
    RandomSource, Registrar, RegistrationContext, ServerHost, Tag,
};

/// The chance that a monster a player killed drops an affix item.
pub const DROP_CHANCE: f32 = 0.075;

/// An affix item's rarity, lowest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Mythic,
}

impl Rarity {
    pub const ALL: [Rarity; 5] = [
        Rarity::Common,
        Rarity::Uncommon,
        Rarity::Rare,
        Rarity::Epic,
        Rarity::Mythic,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Rarity::Common => "common",
            Rarity::Uncommon => "uncommon",
            Rarity::Rare => "rare",
            Rarity::Epic => "epic",
            Rarity::Mythic => "mythic",
        }
    }
    fn from_name(name: &str) -> Option<Self> {
        Rarity::ALL.into_iter().find(|r| r.name() == name)
    }
    /// The name's colour.
    pub fn color(self) -> &'static str {
        match self {
            Rarity::Common => "#808080",
            Rarity::Uncommon => "#33FF33",
            Rarity::Rare => "#5555FF",
            Rarity::Epic => "#BB00BB",
            Rarity::Mythic => "#ED7014",
        }
    }
    /// Weight at luck 0, and how much each point of luck adds (Minecraft's weight + quality ×
    /// luck, as loot table entries roll).
    fn weight(self) -> (i32, i32) {
        match self {
            Rarity::Common => (400, -15),
            Rarity::Uncommon => (320, -5),
            Rarity::Rare => (150, 5),
            Rarity::Epic => (90, 10),
            Rarity::Mythic => (40, 15),
        }
    }
    /// How many affixes an item of this rarity gets.
    pub fn affixes(self) -> usize {
        match self {
            Rarity::Common => 1,
            Rarity::Uncommon => 2,
            Rarity::Rare => 3,
            Rarity::Epic => 4,
            Rarity::Mythic => 5,
        }
    }
    /// From 0 (common) to 1 (mythic): where in an affix's range this rarity's values sit.
    fn step(self) -> f64 {
        self as u8 as f64 / 4.0
    }
}

/// Rolls a rarity, weighted, with `luck` shifting weight to the rarer ones.
pub fn roll_rarity(random: &mut impl RandomSource, luck: f32) -> Rarity {
    let weights = Rarity::ALL.map(|r| {
        let (base, quality) = r.weight();
        (base as f32 + quality as f32 * luck).floor().max(0.0) as i32
    });
    let total: i32 = weights.iter().sum();
    if total <= 0 {
        return Rarity::Common;
    }
    let mut pick = random.next_int_bound(total).unwrap_or(0);
    for (rarity, weight) in Rarity::ALL.into_iter().zip(weights) {
        if pick < weight {
            return rarity;
        }
        pick -= weight;
    }
    Rarity::Common
}

/// What kind of gear an affix fits, and the slots its modifier applies in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Melee,
    Armor(EquipmentSlotGroup),
}

/// One attribute affix: the attribute it modifies, how, and the range of its value at common
/// (`low`) and at mythic (`high`); rarities between sit between.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affix {
    pub id: &'static str,
    pub melee: bool,
    pub attribute: &'static str,
    pub operation: AttributeOperation,
    pub low: (f64, f64),
    pub high: (f64, f64),
}

impl Affix {
    /// The modifier's amount for an item of `rarity` whose affix rolled `level` (0 to 1).
    pub fn value(&self, rarity: Rarity, level: f32) -> f64 {
        let t = rarity.step();
        let min = self.low.0 + (self.high.0 - self.low.0) * t;
        let max = self.low.1 + (self.high.1 - self.low.1) * t;
        min + (max - min) * f64::from(level.clamp(0.0, 1.0))
    }
}

pub const AFFIXES: [Affix; 10] = [
    Affix {
        id: "apotheosis:sword/attribute/violent",
        melee: true,
        attribute: "minecraft:attack_damage",
        operation: AttributeOperation::AddValue,
        low: (0.5, 1.5),
        high: (3.0, 5.0),
    },
    Affix {
        id: "apotheosis:sword/attribute/graceful",
        melee: true,
        attribute: "minecraft:attack_speed",
        operation: AttributeOperation::AddMultipliedBase,
        low: (0.05, 0.1),
        high: (0.2, 0.3),
    },
    Affix {
        id: "apotheosis:sword/attribute/elusive",
        melee: true,
        attribute: "minecraft:movement_speed",
        operation: AttributeOperation::AddMultipliedBase,
        low: (0.02, 0.05),
        high: (0.1, 0.15),
    },
    Affix {
        id: "apotheosis:sword/attribute/forceful",
        melee: true,
        attribute: "minecraft:attack_knockback",
        operation: AttributeOperation::AddValue,
        low: (0.2, 0.4),
        high: (0.8, 1.2),
    },
    Affix {
        id: "apotheosis:sword/attribute/lucky",
        melee: true,
        attribute: "minecraft:luck",
        operation: AttributeOperation::AddValue,
        low: (0.5, 1.0),
        high: (2.0, 3.0),
    },
    Affix {
        id: "apotheosis:armor/attribute/fortified",
        melee: false,
        attribute: "minecraft:armor",
        operation: AttributeOperation::AddValue,
        low: (0.5, 1.0),
        high: (2.0, 4.0),
    },
    Affix {
        id: "apotheosis:armor/attribute/steel_touched",
        melee: false,
        attribute: "minecraft:armor_toughness",
        operation: AttributeOperation::AddValue,
        low: (0.25, 0.5),
        high: (1.5, 3.0),
    },
    Affix {
        id: "apotheosis:armor/attribute/blessed",
        melee: false,
        attribute: "minecraft:max_health",
        operation: AttributeOperation::AddValue,
        low: (1.0, 2.0),
        high: (4.0, 8.0),
    },
    Affix {
        id: "apotheosis:armor/attribute/windswept",
        melee: false,
        attribute: "minecraft:movement_speed",
        operation: AttributeOperation::AddMultipliedBase,
        low: (0.02, 0.04),
        high: (0.08, 0.12),
    },
    Affix {
        id: "apotheosis:armor/attribute/stalwart",
        melee: false,
        attribute: "minecraft:knockback_resistance",
        operation: AttributeOperation::AddValue,
        low: (0.05, 0.1),
        high: (0.2, 0.3),
    },
];

fn affix(id: &str) -> Option<&'static Affix> {
    AFFIXES.iter().find(|a| a.id == id)
}

/// The gear an affix item can be, with its category.
pub const BASES: [(&str, Category); 12] = [
    ("minecraft:iron_sword", Category::Melee),
    ("minecraft:diamond_sword", Category::Melee),
    ("minecraft:iron_axe", Category::Melee),
    ("minecraft:diamond_axe", Category::Melee),
    (
        "minecraft:iron_helmet",
        Category::Armor(EquipmentSlotGroup::Head),
    ),
    (
        "minecraft:iron_chestplate",
        Category::Armor(EquipmentSlotGroup::Chest),
    ),
    (
        "minecraft:iron_leggings",
        Category::Armor(EquipmentSlotGroup::Legs),
    ),
    (
        "minecraft:iron_boots",
        Category::Armor(EquipmentSlotGroup::Feet),
    ),
    (
        "minecraft:diamond_helmet",
        Category::Armor(EquipmentSlotGroup::Head),
    ),
    (
        "minecraft:diamond_chestplate",
        Category::Armor(EquipmentSlotGroup::Chest),
    ),
    (
        "minecraft:diamond_leggings",
        Category::Armor(EquipmentSlotGroup::Legs),
    ),
    (
        "minecraft:diamond_boots",
        Category::Armor(EquipmentSlotGroup::Feet),
    ),
];

/// A rolled affix item, before it becomes a stack.
#[derive(Clone, Debug, PartialEq)]
pub struct Rolled {
    /// Index into [`BASES`].
    pub base: usize,
    pub rarity: Rarity,
    /// Affix id and its level (0 to 1).
    pub affixes: Vec<(&'static str, f32)>,
}

/// Rolls an affix item: its base, rarity, and as many distinct affixes of the base's category
/// as the rarity gives (fewer if the category has fewer).
pub fn roll_item(random: &mut impl RandomSource, luck: f32) -> Rolled {
    let base = random.next_int_bound(BASES.len() as i32).unwrap_or(0) as usize;
    let rarity = roll_rarity(random, luck);
    let melee = BASES[base].1 == Category::Melee;
    let mut pool: Vec<&Affix> = AFFIXES.iter().filter(|a| a.melee == melee).collect();
    let mut affixes = Vec::new();
    while affixes.len() < rarity.affixes() && !pool.is_empty() {
        let i = random.next_int_bound(pool.len() as i32).unwrap_or(0) as usize;
        let chosen = pool.remove(i);
        affixes.push((chosen.id, random.next_float()));
    }
    Rolled {
        base,
        rarity,
        affixes,
    }
}

impl Rolled {
    /// The saved form: `{rarity: "rare", affixes: [{id: "...", level: 0.42f}]}`.
    pub fn to_nbt(&self) -> Compound {
        let affixes = self
            .affixes
            .iter()
            .map(|(id, level)| {
                Tag::Compound(
                    [
                        ("id".into(), Tag::String((*id).into())),
                        ("level".into(), Tag::Float(level.to_bits())),
                    ]
                    .into(),
                )
            })
            .collect();
        [
            ("rarity".into(), Tag::String(self.rarity.name().into())),
            ("affixes".into(), Tag::List(affixes)),
        ]
        .into()
    }
}

/// What a stack's `custom_data.apotheosis` says: its rarity and its affixes with their levels.
pub fn read_affixes(data: &Compound) -> Option<(Rarity, Vec<(&'static Affix, f32)>)> {
    let Some(Tag::String(rarity)) = data.get(&"rarity".into()) else {
        return None;
    };
    let rarity = Rarity::from_name(&rarity.to_string_lossy())?;
    let Some(Tag::List(list)) = data.get(&"affixes".into()) else {
        return Some((rarity, Vec::new()));
    };
    let affixes = list
        .iter()
        .filter_map(|tag| {
            let Tag::Compound(c) = tag else { return None };
            let Some(Tag::String(id)) = c.get(&"id".into()) else {
                return None;
            };
            let level = match c.get(&"level".into()) {
                Some(Tag::Float(bits)) => f32::from_bits(*bits),
                _ => 0.0,
            };
            Some((affix(&id.to_string_lossy())?, level))
        })
        .collect();
    Some((rarity, affixes))
}

fn text(translate: &str, color: Option<&str>, with: Vec<Tag>) -> Tag {
    let mut c: Compound = [("translate".into(), Tag::String(translate.into()))].into();
    if let Some(color) = color {
        c.insert("color".into(), Tag::String(color.into()));
    }
    if !with.is_empty() {
        c.insert("with".into(), Tag::List(with));
    }
    Tag::Compound(c)
}

/// The rolled item as a stack: its custom data and its name, "<Rarity> <item>" in the
/// rarity's colour.
fn stack_of(rolled: &Rolled, item: ItemKey) -> Option<ItemStack> {
    let mut stack = ItemStack::new(item, 1)?;
    let (id, _) = BASES[rolled.base];
    let item_name = format!("item.{}", id.replacen(':', ".", 1));
    let name = text(
        "apotheosis.affix_item",
        Some(rolled.rarity.color()),
        vec![
            text(
                &format!("apotheosis.rarity.{}", rolled.rarity.name()),
                None,
                vec![],
            ),
            text(&item_name, None, vec![]),
        ],
    );
    let patch: Compound = [
        (
            "minecraft:custom_data".into(),
            Tag::Compound([("apotheosis".into(), Tag::Compound(rolled.to_nbt()))].into()),
        ),
        ("minecraft:item_name".into(), name),
    ]
    .into();
    stack.apply_components(&patch).ok()?;
    Some(stack)
}

/// The base items' keys, in [`BASES`] order, known once registration ran.
static BASE_KEYS: OnceLock<Vec<ItemKey>> = OnceLock::new();

fn category_of(item: ItemKey) -> Option<Category> {
    let keys = BASE_KEYS.get()?;
    let i = keys.iter().position(|k| *k == item)?;
    Some(BASES[i].1)
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let keys = BASES.iter().map(|(id, _)| r.item_key(id)).collect();
        let _ = BASE_KEYS.set(keys);
    }

    /// A monster a player killed may drop an affix item.
    fn living_drops(&self, host: &mut dyn ServerHost, event: &mut LivingDropsEvent<'_>) -> Flow {
        if event.cancelled || !event.recently_hit {
            return Flow::Continue;
        }
        let Some(uuid) = event.player else {
            return Flow::Continue;
        };
        if host.entity_type_category(event.kind) != Some(MobCategory::Monster) {
            return Flow::Continue;
        }
        let Some(keys) = BASE_KEYS.get() else {
            return Flow::Continue;
        };
        let Some(entity) = host.entity(event.entity) else {
            return Flow::Continue;
        };
        let Some(level) = entity.level(host) else {
            return Flow::Continue;
        };
        let luck = host
            .player(uuid)
            .and_then(|p| host.living(p.entity()))
            .and_then(|l| l.attribute_value(host, "minecraft:luck"))
            .unwrap_or(0.0) as f32;
        let Some(Some(rolled)) = level.with_random(host, |random| {
            (random.next_float() < DROP_CHANCE).then(|| roll_item(random, luck))
        }) else {
            return Flow::Continue;
        };
        let Some(stack) = stack_of(&rolled, keys[rolled.base]) else {
            return Flow::Continue;
        };
        let position = entity
            .position(host)
            .unwrap_or(event.source.position.unwrap_or([0.0; 3]));
        event
            .drops
            .push(ItemDrop::new(stack, position, [0.0; 3], 10));
        Flow::Continue
    }

    /// The affixes' modifiers, from the stack's recorded rarity and levels.
    fn item_attribute_modifier(&self, event: &mut ItemAttributeModifierEvent<'_>) {
        let Some(category) = category_of(event.stack.item()) else {
            return;
        };
        let Some(Tag::Compound(custom)) = event
            .stack
            .components()
            .and_then(|c| c.as_nbt().get(&"minecraft:custom_data".into()))
        else {
            return;
        };
        let Some(Tag::Compound(data)) = custom.get(&"apotheosis".into()) else {
            return;
        };
        let Some((rarity, affixes)) = read_affixes(data) else {
            return;
        };
        let slot = match category {
            Category::Melee => EquipmentSlotGroup::MainHand,
            Category::Armor(slot) => slot,
        };
        for (affix, level) in affixes {
            let (Some(attribute), Some(id)) = (
                Identifier::parse_namespaced(affix.attribute),
                Identifier::parse_namespaced(affix.id),
            ) else {
                continue;
            };
            let modifier = AttributeModifier::new(id, affix.value(rarity, level), affix.operation);
            event.modifiers.add(attribute, modifier, slot);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mod_api::LegacyRandom;

    #[test]
    fn an_affix_value_spans_its_range_and_grows_with_rarity() {
        let violent = affix("apotheosis:sword/attribute/violent").unwrap();
        assert_eq!(violent.value(Rarity::Common, 0.0), 0.5);
        assert_eq!(violent.value(Rarity::Common, 1.0), 1.5);
        assert_eq!(violent.value(Rarity::Mythic, 0.0), 3.0);
        assert_eq!(violent.value(Rarity::Mythic, 1.0), 5.0);
        // Rare sits half way: min 1.75, max 3.25.
        assert_eq!(violent.value(Rarity::Rare, 0.5), 2.5);
    }

    #[test]
    fn luck_shifts_rolls_toward_rarer_items() {
        let count = |luck: f32| {
            let mut random = LegacyRandom::new(42);
            (0..10_000)
                .filter(|_| roll_rarity(&mut random, luck) >= Rarity::Epic)
                .count()
        };
        // Weights 400/320/150/90/40 put 130 of 1000 at epic or above; luck 10 makes it
        // 190 + 190 of 1100 (common 250, uncommon 270, rare 200, epic 190, mythic 190).
        let plain = count(0.0);
        let lucky = count(10.0);
        assert!((1_100..1_500).contains(&plain), "{plain}");
        assert!((3_200..3_700).contains(&lucky), "{lucky}");
    }

    #[test]
    fn a_rolled_item_has_distinct_affixes_of_its_category_counted_by_rarity() {
        let mut random = LegacyRandom::new(7);
        for _ in 0..500 {
            let rolled = roll_item(&mut random, 0.0);
            let melee = BASES[rolled.base].1 == Category::Melee;
            assert_eq!(rolled.affixes.len(), rolled.rarity.affixes());
            let mut ids: Vec<_> = rolled.affixes.iter().map(|a| a.0).collect();
            ids.sort();
            ids.dedup();
            assert_eq!(ids.len(), rolled.affixes.len());
            assert!(rolled.affixes.iter().all(|(id, level)| {
                affix(id).unwrap().melee == melee && (0.0..1.0).contains(level)
            }));
        }
    }

    #[test]
    fn the_saved_form_reads_back() {
        let rolled = Rolled {
            base: 5,
            rarity: Rarity::Epic,
            affixes: vec![
                ("apotheosis:armor/attribute/blessed", 0.25),
                ("apotheosis:armor/attribute/stalwart", 0.75),
            ],
        };
        let (rarity, affixes) = read_affixes(&rolled.to_nbt()).unwrap();
        assert_eq!(rarity, Rarity::Epic);
        let read: Vec<_> = affixes.iter().map(|(a, l)| (a.id, *l)).collect();
        assert_eq!(read, rolled.affixes);
    }
}
