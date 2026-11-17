//! Iron Furnaces for Horizon: furnaces in tiers that smelt faster than vanilla's.
//!
//! Written from Iron Furnaces' observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and what a harness against the NeoForge
//! original still has to confirm, is in README.md.
//!
//! Each tier is a block on the vanilla furnace's template (its `facing` and `lit` states) with a
//! block entity of the one type `ironfurnaces:furnace`. Its three slots (input, fuel, result) open
//! the vanilla furnace menu, whose four data slots show the flame and the arrow. The tick is
//! vanilla's `AbstractFurnaceBlockEntity.serverTick`, with the tier's cook time in place of the
//! recipe's and the fuel's burn time scaled by the same factor, so one coal still smelts eight items.
use std::sync::OnceLock;

use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockKey,
    BlockPart, Compound, Hooks, Host, Identifier, ItemStack, LevelSide, ModContainer, RecipeInput,
    Registrar, RegistrationContext, ServerHost, Tag,
};

/// The tiers, in the order of [`TIERS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tier {
    /// The block's path in the `ironfurnaces` namespace.
    pub name: &'static str,
    /// Ticks to smelt one item (vanilla's furnace: 200).
    pub cook_time: i32,
}

/// Iron Furnaces' default cook times (its server config). Values to confirm on the original.
pub const TIERS: [Tier; 13] = [
    Tier { name: "copper_furnace", cook_time: 180 },
    Tier { name: "iron_furnace", cook_time: 160 },
    Tier { name: "silver_furnace", cook_time: 140 },
    Tier { name: "gold_furnace", cook_time: 120 },
    Tier { name: "diamond_furnace", cook_time: 80 },
    Tier { name: "emerald_furnace", cook_time: 40 },
    Tier { name: "crystal_furnace", cook_time: 40 },
    Tier { name: "obsidian_furnace", cook_time: 20 },
    Tier { name: "netherite_furnace", cook_time: 5 },
    Tier { name: "allthemodium_furnace", cook_time: 5 },
    Tier { name: "vibranium_furnace", cook_time: 3 },
    Tier { name: "unobtainium_furnace", cook_time: 1 },
    Tier { name: "million_furnace", cook_time: 20 },
];

/// Vanilla's furnace cook time, the base a fuel's burn time is given for.
pub const VANILLA_COOK_TIME: i32 = 200;

/// Slots, as vanilla's furnace numbers them.
pub const INPUT: usize = 0;
pub const FUEL: usize = 1;
pub const RESULT: usize = 2;

/// The furnace's progress, saved in its compound (menu data is not saved).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    /// Ticks the current fuel still burns (`litTimeRemaining`).
    pub lit_time: i32,
    /// Ticks the current fuel burned in all (`litTotalTime`).
    pub lit_duration: i32,
    /// Ticks spent on the current item (`cookingTimer`).
    pub cooking: i32,
    /// Ticks the current item needs (`cookingTotalTime`).
    pub cooking_total: i32,
}

impl Progress {
    pub fn lit(&self) -> bool {
        self.lit_time > 0
    }

    pub fn load(nbt: &Compound) -> Self {
        let int = |key: &str| match nbt.get(&key.into()) {
            Some(Tag::Int(v)) => *v,
            Some(Tag::Short(v)) => i32::from(*v),
            _ => 0,
        };
        Self {
            lit_time: int("lit_time_remaining"),
            lit_duration: int("lit_total_time"),
            cooking: int("cooking_time_spent"),
            cooking_total: int("cooking_total_time"),
        }
    }

    pub fn save(&self, nbt: &mut Compound) {
        for (key, value) in [
            ("lit_time_remaining", self.lit_time),
            ("lit_total_time", self.lit_duration),
            ("cooking_time_spent", self.cooking),
            ("cooking_total_time", self.cooking_total),
        ] {
            nbt.insert(key.into(), Tag::Int(value));
        }
    }

    /// The vanilla furnace menu's four data slots.
    pub fn data(&self) -> [i32; 4] {
        [self.lit_time, self.lit_duration, self.cooking, self.cooking_total]
    }
}

/// What one tick decided, for the glue to carry out on the slots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Step {
    /// Take one item from the fuel slot (leaving its remainder, a bucket's bucket).
    pub consume_fuel: bool,
    /// Smelt one item: take one input, add the result.
    pub smelt: bool,
}

/// A fuel's burn time on a tier: vanilla's, scaled by the tier's speed, never below 1 tick.
pub fn scaled_burn_time(vanilla_burn: i32, cook_time: i32) -> i32 {
    if vanilla_burn <= 0 {
        return 0;
    }
    ((i64::from(vanilla_burn) * i64::from(cook_time)) / i64::from(VANILLA_COOK_TIME)).max(1) as i32
}

/// One server tick of a furnace (`AbstractFurnaceBlockEntity.serverTick`).
///
/// `can_burn`: there is input with a recipe whose result fits the result slot.
/// `fuel_burn`: the scaled burn time of the item in the fuel slot, if it is fuel.
/// `has_input`: the input slot holds something.
pub fn tick(p: &mut Progress, cook_time: i32, has_input: bool, can_burn: bool, fuel_burn: Option<i32>) -> Step {
    let mut step = Step::default();
    let was_lit = p.lit();
    if was_lit {
        p.lit_time -= 1;
    }
    let has_fuel = fuel_burn.is_some_and(|b| b > 0);
    if (p.lit() || has_fuel) && has_input {
        if !p.lit() && can_burn {
            if let Some(burn) = fuel_burn.filter(|b| *b > 0) {
                p.lit_time = burn;
                p.lit_duration = burn;
                step.consume_fuel = true;
            }
        }
        if p.lit() && can_burn {
            p.cooking_total = cook_time;
            p.cooking += 1;
            if p.cooking >= p.cooking_total {
                p.cooking = 0;
                step.smelt = true;
            }
        } else {
            p.cooking = 0;
        }
    } else if !p.lit() && p.cooking > 0 {
        p.cooking = (p.cooking - 2).clamp(0, p.cooking_total.max(0));
    }
    step
}

/// Whether `result` can go into a result slot holding `current`.
fn fits(host: &dyn Host, current: &Option<ItemStack>, result: &ItemStack) -> bool {
    match current {
        None => true,
        Some(held) => {
            let max = host.max_stack_size(held.item()).unwrap_or(64);
            held.is_stackable_with(result) && held.count() + result.count() <= max
        }
    }
}

/// The blocks' keys, in [`TIERS`] order, known once registration ran.
static KEYS: OnceLock<Vec<BlockKey>> = OnceLock::new();

fn tier_of(block: BlockKey) -> Option<Tier> {
    let keys = KEYS.get()?;
    keys.iter().position(|k| *k == block).map(|i| TIERS[i])
}

fn is_ours(block: &str) -> bool {
    block
        .strip_prefix("ironfurnaces:")
        .is_some_and(|name| TIERS.iter().any(|t| t.name == name))
}

struct Furnace;

#[mod_api::horizon_part]
impl BlockPart for Furnace {}

#[mod_api::horizon_part]
impl BlockEntityPart for Furnace {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        is_ours(event.block).then(|| {
            let mut nbt = Compound::new();
            Progress::default().save(&mut nbt);
            nbt
        })
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if is_ours(event.block) {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        let Some(tier) = level.block(host, pos).and_then(tier_of) else {
            return;
        };
        if event.entity.items.len() < 3 {
            event.entity.items.resize(3, None);
        }
        let mut progress = Progress::load(&event.entity.nbt);
        let input = event.entity.items[INPUT].clone();
        let fuel = event.entity.items[FUEL].clone();
        let result = event.entity.items[RESULT].clone();

        let recipe = input.as_ref().and_then(|stack| {
            let kind = Identifier::parse("minecraft:smelting").ok()?;
            level
                .recipe_for(host, &kind, &RecipeInput::Single(stack.clone()))
                .ok()
                .flatten()
        });
        let can_burn = recipe
            .as_ref()
            .is_some_and(|m| fits(host, &result, &m.output));
        let fuel_burn = fuel
            .as_ref()
            .and_then(|f| host.item_burn_time(f.item()))
            .map(|b| scaled_burn_time(b, tier.cook_time));

        let before = progress;
        let step = tick(&mut progress, tier.cook_time, input.is_some(), can_burn, fuel_burn);

        if step.consume_fuel {
            if let Some(f) = fuel {
                let remainder = host.item_crafting_remainder(&f);
                event.entity.items[FUEL] = f.shrink(1).or(remainder);
            }
        }
        if step.smelt {
            if let (Some(m), Some(i)) = (recipe, input) {
                event.entity.items[RESULT] = match result {
                    None => Some(m.output.clone()),
                    Some(mut held) => {
                        let _ = held.grow(m.output.count());
                        Some(held)
                    }
                };
                event.entity.items[INPUT] = i.shrink(1);
            }
        }

        if before == progress && !step.consume_fuel && !step.smelt {
            *event.reply = mod_api::BlockEntityTickReply::Unchanged;
            return;
        }
        progress.save(&mut event.entity.nbt);
        let data = progress.data();
        if event.entity.data.len() != data.len() {
            event.entity.data = data.to_vec();
        } else {
            event.entity.data.copy_from_slice(&data);
        }
        if before.lit() != progress.lit() {
            host.set_block_property(pos, "lit", if progress.lit() { "true" } else { "false" });
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let ids: Vec<String> = TIERS.iter().map(|t| format!("ironfurnaces:{}", t.name)).collect();
        let names: Vec<&str> = ids.iter().map(String::as_str).collect();
        let keys: Vec<BlockKey> = names.iter().map(|id| r.block_key(id)).collect();
        for key in &keys {
            r.block_container(
                key,
                ModContainer {
                    slots: 3,
                    // down, up, north, south, west, east: as vanilla's furnace
                    faces: [
                        vec![RESULT, FUEL],
                        vec![INPUT],
                        vec![FUEL],
                        vec![FUEL],
                        vec![FUEL],
                        vec![FUEL],
                    ],
                    inputs: vec![INPUT, FUEL],
                    outputs: vec![RESULT],
                    menu: Some("minecraft:furnace".into()),
                    slot_limit: 0,
                },
            );
        }
        let _ = KEYS.set(keys);
        let kind = r.block_entity_type("furnace").blocks(&names).build();
        let _ = r.block_entity_part(&kind, Furnace);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(p: &mut Progress, ticks: usize, cook: i32, can_burn: bool, fuel: Option<i32>) -> (usize, usize) {
        let (mut fuels, mut smelts) = (0, 0);
        let mut fuel_left = fuel;
        for _ in 0..ticks {
            let s = tick(p, cook, true, can_burn, fuel_left);
            if s.consume_fuel {
                fuels += 1;
            }
            if s.smelt {
                smelts += 1;
            }
            let _ = &mut fuel_left;
        }
        (fuels, smelts)
    }

    #[test]
    fn an_iron_furnace_smelts_one_item_in_160_ticks() {
        let mut p = Progress::default();
        let coal = scaled_burn_time(1600, 160);
        let (fuels, smelts) = run(&mut p, 160, 160, true, Some(coal));
        assert_eq!((fuels, smelts), (1, 1));
        let (_, smelts) = run(&mut p, 159, 160, true, Some(coal));
        assert_eq!(smelts, 0);
    }

    #[test]
    fn one_coal_smelts_eight_items_on_every_tier() {
        for tier in TIERS {
            let burn = scaled_burn_time(1600, tier.cook_time);
            assert_eq!(burn / tier.cook_time, 8, "{}", tier.name);
        }
    }

    #[test]
    fn nothing_burns_without_a_recipe_and_progress_falls_back_when_unlit() {
        let mut p = Progress::default();
        let s = tick(&mut p, 160, true, false, Some(1280));
        assert_eq!(s, Step::default());
        assert!(!p.lit());
        let mut p = Progress { cooking: 5, cooking_total: 160, ..Default::default() };
        tick(&mut p, 160, false, false, None);
        assert_eq!(p.cooking, 3);
    }

    #[test]
    fn progress_round_trips_through_its_saved_form() {
        let p = Progress { lit_time: 7, lit_duration: 1280, cooking: 33, cooking_total: 160 };
        let mut nbt = Compound::new();
        p.save(&mut nbt);
        assert_eq!(Progress::load(&nbt), p);
    }
}
