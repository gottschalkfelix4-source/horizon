//! Baubley Heart Canisters for Horizon: hostile mobs and bosses drop hearts, hearts make
//! canisters, and canisters in a Heart Amulet each add a heart of maximum health.
//!
//! Written from the mod's public description and observed behaviour, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and what a harness against the NeoForge
//! original still has to confirm, is in README.md.
//!
//! The original wears the amulet in a Curios necklace slot. Horizon has no accessory slots yet
//! (README, "Needs from Horizon"), so here the amulet counts while it is anywhere in the player's
//! inventory, the first one found.

pub mod hearts;

use std::sync::OnceLock;

use mod_api::{
    AttributeModifier, AttributeOperation, ClientHost, Component, ComponentTypesEvent, Flow, Hand,
    Hooks, HostSide, Identifier, InteractionResult, ItemDrop, ItemKey, ItemPart, ItemStack,
    LivingDropsEvent, MobCategory, ModifierLifetime, PlayerInventoryChangedEvent, PlayerRef,
    PlayerRespawnedEvent, RandomSource, Registrar, RegistrationContext, ServerHost, Tag, Text,
    TooltipEvent, UseItemEvent,
};

use hearts::{Colour, Counts, MAX_PER_COLOUR, bonus_health, heart_drop, insert};

/// The amulet's canisters, an int array `[red, orange, green, blue]`.
pub struct CanistersComponent;

pub const CANISTERS: CanistersComponent = CanistersComponent;

impl Component for CanistersComponent {
    type Value = Counts;
    fn id(&self) -> &str {
        "bhc:canisters"
    }
    fn decode(&self, value: &Tag) -> Option<Counts> {
        match value {
            Tag::IntArray(v) if v.len() == 4 => Some([v[0], v[1], v[2], v[3]]),
            _ => None,
        }
    }
    fn encode(&self, value: Counts) -> Tag {
        Tag::IntArray(value.to_vec())
    }
}

/// The modifier the amulet puts on `minecraft:max_health`.
const MODIFIER: &str = "bhc:heart_canisters";

struct Keys {
    amulet: ItemKey,
    hearts: [ItemKey; 4],
    canisters: [ItemKey; 4],
}

static KEYS: OnceLock<Keys> = OnceLock::new();

fn canister_colour(item: ItemKey) -> Option<Colour> {
    let keys = KEYS.get()?;
    Colour::ALL
        .into_iter()
        .find(|c| keys.canisters[c.index()] == item)
}

fn is_amulet(stack: &ItemStack) -> bool {
    KEYS.get().is_some_and(|k| k.amulet == stack.item())
}

fn counts_of(stack: &ItemStack) -> Counts {
    stack.get(&CANISTERS).unwrap_or([0; 4])
}

/// The first amulet's counts among `stacks`.
fn worn_counts<'a>(stacks: impl IntoIterator<Item = &'a ItemStack>) -> Counts {
    stacks
        .into_iter()
        .find(|s| is_amulet(s))
        .map_or([0; 4], counts_of)
}

/// Sets the player's max health bonus to what `counts` give.
fn apply_health(host: &mut dyn ServerHost, player: PlayerRef, counts: Counts) {
    let Some(living) = host.living(player.entity()) else {
        return;
    };
    let Ok(id) = Identifier::parse(MODIFIER) else {
        return;
    };
    let bonus = bonus_health(counts);
    if bonus > 0.0 {
        let modifier = AttributeModifier::new(id, bonus, AttributeOperation::AddValue);
        living.replace_attribute_modifier(
            host,
            "minecraft:max_health",
            modifier,
            ModifierLifetime::Transient,
        );
    } else {
        living.remove_attribute_modifier(host, "minecraft:max_health", &id);
    }
    // Vanilla clamps health to a lowered max only when it is next set.
    if let (Some(health), Some(max)) = (living.health(host), living.max_health(host))
        && health > max
    {
        living.set_health(host, max);
    }
}

/// A canister: used with the amulet in the other hand, it goes into the amulet.
struct CanisterItem;

#[mod_api::horizon_part]
impl ItemPart for CanisterItem {
    fn use_item(&self, host: HostSide, event: &mut UseItemEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        let colour = canister_colour(event.stack.item())?;
        let player = host.player(event.player)?;
        let living = host.living(player.entity())?;
        let other = match event.hand {
            Hand::Main => Hand::Off,
            Hand::Off => Hand::Main,
        };
        let amulet = living.hand_item(host, other).filter(is_amulet)?;
        let Some(counts) = insert(counts_of(&amulet), colour) else {
            let _ = player.action_bar(host, Text::translate("bhc.amulet.full", Vec::new()));
            return Some(InteractionResult::Fail);
        };
        let mut amulet = amulet;
        amulet.set(&CANISTERS, counts).ok()?;
        let _ = living.set_hand_item(host, other, Some(amulet));
        let _ = living.set_hand_item(host, event.hand, event.stack.clone().shrink(1));
        apply_health(host, player, counts);
        Some(InteractionResult::Success)
    }
}

/// The amulet's tooltip: the canisters it holds.
struct AmuletItem;

#[mod_api::horizon_part]
impl ItemPart for AmuletItem {
    fn tooltip(&self, _: &dyn ClientHost, event: &mut TooltipEvent<'_>) {
        let counts = counts_of(event.stack);
        for colour in Colour::ALL {
            let n = counts[colour.index()];
            if n == 0 {
                continue;
            }
            let line = Text::translate(
                "bhc.tooltip.canisters",
                vec![
                    Text::literal(n.to_string()),
                    Text::literal(MAX_PER_COLOUR.to_string()),
                    Text::translate(format!("bhc.colour.{}", colour.name()), Vec::new()),
                ],
            );
            event.lines.push((line, 0xAA_AA_AA));
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let item = |r: &mut dyn Registrar, suffix: &str| {
            Colour::ALL.map(|c| r.item_key(&format!("bhc:{}_{suffix}", c.name())))
        };
        let keys = Keys {
            amulet: r.item_key("bhc:heart_amulet"),
            hearts: item(r, "heart"),
            canisters: item(r, "heart_canister"),
        };
        for canister in keys.canisters {
            r.item_part(canister, CanisterItem);
        }
        r.item_part(keys.amulet, AmuletItem);
        let _ = KEYS.set(keys);
    }

    fn component_types(&self, _: &RegistrationContext, event: &mut ComponentTypesEvent<'_>) {
        event.types.push(CANISTERS.id().to_string());
    }

    /// Follow the amulet: carrying one, or changing what it holds, changes max health.
    fn player_inventory_changed(
        &self,
        host: &mut dyn ServerHost,
        event: &mut PlayerInventoryChangedEvent<'_>,
    ) {
        let Some(player) = host.player(event.player) else {
            return;
        };
        let counts = worn_counts(event.stacks.iter().flatten());
        apply_health(host, player, counts);
    }

    /// A respawned player is a new entity without the transient modifier.
    fn player_respawned(&self, host: &mut dyn ServerHost, event: &mut PlayerRespawnedEvent<'_>) {
        let Some(player) = host.player(event.player) else {
            return;
        };
        let stacks = player.inventory(host).unwrap_or_default();
        apply_health(host, player, worn_counts(&stacks));
    }

    /// A mob killed by a player may drop a heart.
    fn living_drops(&self, host: &mut dyn ServerHost, event: &mut LivingDropsEvent<'_>) -> Flow {
        if event.player.is_none() {
            return Flow::Continue;
        }
        let hostile = host.entity_type_category(event.kind) == Some(MobCategory::Monster);
        let Some((colour, chance)) = heart_drop(event.kind, hostile) else {
            return Flow::Continue;
        };
        let Some(entity) = host.entity(event.entity) else {
            return Flow::Continue;
        };
        let (Some(level), Some(at)) = (entity.level(host), entity.position(host)) else {
            return Flow::Continue;
        };
        let roll = if chance >= 1.0 {
            Some(0.0)
        } else {
            level.with_random(host, |r| r.next_float())
        };
        if roll.is_some_and(|r| r < chance)
            && let Some(keys) = KEYS.get()
        {
            let heart = ItemStack::of(keys.hearts[colour.index()]);
            event.drops.push(ItemDrop::new(heart, at, [0.0; 3], 10));
        }
        Flow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canister_counts_round_trip_and_refuse_other_shapes() {
        let counts = [1, 0, 10, 3];
        assert_eq!(CANISTERS.decode(&CANISTERS.encode(counts)), Some(counts));
        assert_eq!(CANISTERS.decode(&Tag::IntArray(vec![1, 2])), None);
    }
}
