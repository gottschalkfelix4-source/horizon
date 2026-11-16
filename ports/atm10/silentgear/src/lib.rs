//! Silent Gear for Horizon, first slice: gear stats from materials. A Silent Gear tool is made of
//! parts, and its durability, mining speed, harvest tier and attack damage come from the
//! materials of its main part; mixing materials gives a synergy bonus or penalty. Silent Gear
//! tools never break: they stop one point short and wait for a repair.
//!
//! Written from Silent Gear's observed behaviour and public description, never from its code or
//! data (docs/modpacks.md, "Licences"). Every material number and the synergy rule are from
//! memory and have to be checked against the original: see README.md.
//!
//! How it works: a gear stack records what it is made of in `minecraft:custom_data` under
//! `silentgear` (`{materials: ["iron", "iron", "diamond"], rod: "wood"}`). Its stats are computed
//! from that record ([`gear_stats`]):
//! - `item_attribute_modifier` gives it the attack damage and speed modifiers (by vanilla's ids,
//!   so they replace the template's);
//! - while a player carries it, `inventory_tick` writes `minecraft:max_damage` and, for a
//!   pickaxe, `minecraft:tool` (mining speed and harvest tier) once, when they differ from the
//!   computed ones;
//! - `damage_item` keeps it from breaking.
use std::sync::OnceLock;

use mod_api::{
    AttributeModifier, AttributeOperation, Compound, DamageItemEvent, EquipmentSlotGroup, Flow,
    Hooks, HostTransaction, Identifier, InventoryTickEvent, ItemAttributeModifierEvent, ItemKey,
    ItemPart, ItemPlace, ItemStack, Registrar, RegistrationContext, ServerHost, Tag,
};

/// One material's main-part stats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    pub id: &'static str,
    pub durability: f32,
    /// 0 wood or gold, 1 stone, 2 iron, 3 diamond, 4 netherite.
    pub tier: u8,
    pub speed: f32,
    pub damage: f32,
    pub rarity: f32,
}

const fn mat(
    id: &'static str,
    durability: f32,
    tier: u8,
    speed: f32,
    damage: f32,
    rarity: f32,
) -> Material {
    Material {
        id,
        durability,
        tier,
        speed,
        damage,
        rarity,
    }
}

pub const MATERIALS: [Material; 8] = [
    mat("wood", 59.0, 0, 2.0, 0.0, 0.0),
    mat("stone", 131.0, 1, 4.0, 1.0, 5.0),
    mat("flint", 124.0, 1, 5.0, 1.0, 10.0),
    mat("iron", 250.0, 2, 6.0, 2.0, 20.0),
    mat("gold", 32.0, 0, 12.0, 0.0, 40.0),
    mat("emerald", 1080.0, 2, 10.0, 2.0, 60.0),
    mat("diamond", 1561.0, 3, 8.0, 3.0, 70.0),
    mat("netherite", 2031.0, 4, 9.0, 4.0, 80.0),
];

pub fn material(id: &str) -> Option<&'static Material> {
    MATERIALS.iter().find(|m| m.id == id)
}

/// The gear kinds this slice registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Pickaxe,
    Sword,
}

impl Kind {
    pub const ALL: [Kind; 2] = [Kind::Pickaxe, Kind::Sword];
    fn id(self) -> &'static str {
        match self {
            Kind::Pickaxe => "silentgear:pickaxe",
            Kind::Sword => "silentgear:sword",
        }
    }
    /// Attack damage the kind adds to its material's, and its attack speed modifier.
    fn attack(self) -> (f32, f32) {
        match self {
            Kind::Pickaxe => (1.0, -2.8),
            Kind::Sword => (3.0, -2.4),
        }
    }
}

/// A gear item's computed stats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    pub durability: u32,
    pub tier: u8,
    pub speed: f32,
    /// The attack damage modifier (on top of the player's 1).
    pub damage: f32,
    pub attack_speed: f32,
}

/// Synergy of a main part's materials: 1 for one material, +10 % for each further distinct
/// material, -0.5 % per point between the rarest and the commonest; kept in 0.5 to 1.5.
pub fn synergy(materials: &[&Material]) -> f32 {
    let mut ids: Vec<&str> = materials.iter().map(|m| m.id).collect();
    ids.sort_unstable();
    ids.dedup();
    let rarities = materials.iter().map(|m| m.rarity);
    let spread = rarities.clone().fold(f32::MIN, f32::max) - rarities.fold(f32::MAX, f32::min);
    (1.0 + 0.1 * (ids.len() as f32 - 1.0) - 0.005 * spread).clamp(0.5, 1.5)
}

/// A gear item's stats from its main part's materials: durability, speed and damage are the
/// materials' average times the synergy, the tier is the highest material's.
pub fn gear_stats(kind: Kind, materials: &[&Material]) -> Option<Stats> {
    if materials.is_empty() {
        return None;
    }
    let n = materials.len() as f32;
    let avg = |f: fn(&Material) -> f32| materials.iter().map(|m| f(m)).sum::<f32>() / n;
    let syn = synergy(materials);
    let (base_damage, attack_speed) = kind.attack();
    Some(Stats {
        durability: (avg(|m| m.durability) * syn).round().max(1.0) as u32,
        tier: materials.iter().map(|m| m.tier).max().unwrap_or(0),
        speed: avg(|m| m.speed) * syn,
        damage: base_damage + avg(|m| m.damage) * syn,
        attack_speed,
    })
}

/// How much wear to let through so that a gear item with `damage` of `max` never reaches its
/// break: it stops at `max - 1`.
pub fn allowed_wear(damage: u32, max: u32, amount: i32) -> i32 {
    let left = i64::from(max) - 1 - i64::from(damage);
    i64::from(amount).min(left.max(0)) as i32
}

/// Whether a gear item is broken (Silent Gear's state at one point from breaking).
pub fn is_broken(damage: u32, max: u32) -> bool {
    max > 0 && damage + 1 >= max
}

/// The materials a stack records, if it is a gear stack with a valid record.
fn materials_of(stack: &ItemStack) -> Option<Vec<&'static Material>> {
    let Some(Tag::Compound(custom)) = stack
        .components()?
        .as_nbt()
        .get(&"minecraft:custom_data".into())
    else {
        return None;
    };
    let Some(Tag::Compound(gear)) = custom.get(&"silentgear".into()) else {
        return None;
    };
    read_materials(gear)
}

/// `{materials: ["iron", ...]}` as materials; None for an unknown one or none at all.
pub fn read_materials(gear: &Compound) -> Option<Vec<&'static Material>> {
    let Some(Tag::List(list)) = gear.get(&"materials".into()) else {
        return None;
    };
    let materials = list
        .iter()
        .map(|t| match t {
            Tag::String(s) => material(&s.to_string_lossy()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    (!materials.is_empty()).then_some(materials)
}

const TIER_TAGS: [&str; 5] = [
    "#minecraft:incorrect_for_wooden_tool",
    "#minecraft:incorrect_for_stone_tool",
    "#minecraft:incorrect_for_iron_tool",
    "#minecraft:incorrect_for_diamond_tool",
    "#minecraft:incorrect_for_netherite_tool",
];

/// The pickaxe's `minecraft:tool`: nothing below its tier drops, mineable blocks at its speed.
fn tool_component(stats: &Stats) -> Tag {
    let rule = |blocks: &str, speed: Option<f32>, correct: bool| {
        let mut c: Compound = [
            ("blocks".into(), Tag::String(blocks.into())),
            ("correct_for_drops".into(), Tag::Byte(i8::from(correct))),
        ]
        .into();
        if let Some(speed) = speed {
            c.insert("speed".into(), Tag::Float(speed.to_bits()));
        }
        Tag::Compound(c)
    };
    let tier = usize::from(stats.tier.min(4));
    Tag::Compound(
        [
            (
                "rules".into(),
                Tag::List(vec![
                    rule(TIER_TAGS[tier], None, false),
                    rule("#minecraft:mineable/pickaxe", Some(stats.speed), true),
                ]),
            ),
            ("default_mining_speed".into(), Tag::Float(1.0f32.to_bits())),
            ("damage_per_block".into(), Tag::Int(1)),
        ]
        .into(),
    )
}

/// The components a gear stack should carry for its stats.
fn wanted_components(kind: Kind, stats: &Stats) -> Compound {
    let mut patch: Compound = [(
        "minecraft:max_damage".into(),
        Tag::Int(stats.durability as i32),
    )]
    .into();
    if kind == Kind::Pickaxe {
        patch.insert("minecraft:tool".into(), tool_component(stats));
    }
    patch
}

static KEYS: OnceLock<Vec<ItemKey>> = OnceLock::new();

fn kind_of(item: ItemKey) -> Option<Kind> {
    let i = KEYS.get()?.iter().position(|k| *k == item)?;
    Some(Kind::ALL[i])
}

fn max_damage(stack: &ItemStack) -> Option<u32> {
    match stack
        .components()?
        .as_nbt()
        .get(&"minecraft:max_damage".into())
    {
        Some(Tag::Int(v)) => u32::try_from(*v).ok(),
        _ => None,
    }
}

/// The gear items: their components follow their materials, and they never break.
struct Gear;

#[mod_api::horizon_part]
impl ItemPart for Gear {
    fn inventory_tick(&self, host: &mut dyn ServerHost, event: &mut InventoryTickEvent<'_>) {
        let Some(kind) = kind_of(event.stack.item()) else {
            return;
        };
        let Some(stats) = materials_of(event.stack).and_then(|m| gear_stats(kind, &m)) else {
            return;
        };
        // `max_damage` alone decides: the game may store the tool rules in another (equal)
        // form, and a comparison of those would rewrite the stack every tick.
        if max_damage(event.stack) == Some(stats.durability) {
            return;
        }
        let wanted = wanted_components(kind, &stats);
        let Some(player) = host.players().acting_player() else {
            return;
        };
        let place = ItemPlace::PlayerSlot {
            player: player.uuid(),
            slot: event.slot,
        };
        let _ = host.transaction(|host, tx| {
            if place.edit(host, tx, |stack| {
                let _ = stack.apply_components(&wanted);
            }) {
                Ok(())
            } else {
                Err(())
            }
        });
    }

    fn damage_item(&self, _: &mut dyn ServerHost, event: &mut DamageItemEvent<'_>) -> Flow {
        if kind_of(event.stack.item()).is_none() {
            return Flow::Continue;
        }
        let Some(max) = max_damage(event.stack) else {
            return Flow::Continue;
        };
        event.amount = allowed_wear(event.stack.damage(), max, event.amount);
        event.break_now = false;
        Flow::Continue
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let keys: Vec<ItemKey> = Kind::ALL.iter().map(|k| r.item_key(k.id())).collect();
        for key in &keys {
            r.item_part(*key, Gear);
        }
        let _ = KEYS.set(keys);
    }

    /// Attack damage and speed from the materials; a broken one hits like a fist.
    fn item_attribute_modifier(&self, event: &mut ItemAttributeModifierEvent<'_>) {
        let Some(kind) = kind_of(event.stack.item()) else {
            return;
        };
        let Some(stats) = materials_of(event.stack).and_then(|m| gear_stats(kind, &m)) else {
            return;
        };
        let broken =
            max_damage(event.stack).is_some_and(|max| is_broken(event.stack.damage(), max));
        let damage = if broken { 0.0 } else { stats.damage };
        for (attribute, id, amount) in [
            (
                "minecraft:attack_damage",
                "minecraft:base_attack_damage",
                damage,
            ),
            (
                "minecraft:attack_speed",
                "minecraft:base_attack_speed",
                stats.attack_speed,
            ),
        ] {
            let (Some(attribute), Some(id)) = (
                Identifier::parse_namespaced(attribute),
                Identifier::parse_namespaced(id),
            ) else {
                continue;
            };
            let modifier =
                AttributeModifier::new(id, f64::from(amount), AttributeOperation::AddValue);
            event
                .modifiers
                .replace(attribute, modifier, EquipmentSlotGroup::MainHand);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mats(ids: &[&str]) -> Vec<&'static Material> {
        ids.iter().map(|id| material(id).unwrap()).collect()
    }

    #[test]
    fn single_material_gear_matches_its_vanilla_counterpart() {
        let pick = gear_stats(Kind::Pickaxe, &mats(&["iron", "iron"])).unwrap();
        assert_eq!(pick.durability, 250);
        assert_eq!(pick.tier, 2);
        assert_eq!(pick.speed, 6.0);
        // Vanilla's iron pickaxe hits for 4: the player's 1 and a modifier of 3.
        assert_eq!(pick.damage, 3.0);
        let sword = gear_stats(Kind::Sword, &mats(&["diamond"])).unwrap();
        assert_eq!(sword.durability, 1561);
        // Vanilla's diamond sword hits for 7.
        assert_eq!(sword.damage, 6.0);
    }

    #[test]
    fn mixed_materials_average_with_synergy_and_take_the_highest_tier() {
        // Iron and diamond: two materials (+0.1), rarities 20 and 70 (-0.25): synergy 0.85.
        let pick = gear_stats(Kind::Pickaxe, &mats(&["iron", "diamond"])).unwrap();
        assert_eq!(pick.tier, 3);
        // (250 + 1561) / 2 = 905.5, times 0.85 = 769.675.
        assert_eq!(pick.durability, 770);
        // (6 + 8) / 2 * 0.85 = 5.95.
        assert!((pick.speed - 5.95).abs() < 1e-5);
    }

    #[test]
    fn gear_stops_one_point_short_of_breaking() {
        assert_eq!(allowed_wear(0, 250, 1), 1);
        assert_eq!(allowed_wear(248, 250, 5), 1);
        assert_eq!(allowed_wear(249, 250, 1), 0);
        assert!(is_broken(249, 250));
        assert!(!is_broken(248, 250));
    }

    #[test]
    fn a_record_naming_an_unknown_material_is_no_gear() {
        let gear: Compound = [(
            "materials".into(),
            Tag::List(vec![
                Tag::String("iron".into()),
                Tag::String("unobtainium".into()),
            ]),
        )]
        .into();
        assert!(read_materials(&gear).is_none());
        let empty: Compound = [("materials".into(), Tag::List(vec![]))].into();
        assert!(read_materials(&empty).is_none());
    }
}
