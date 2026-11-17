//! Step Crafter for Horizon: a crafting block that crafts one result each time it is powered.
//!
//! Written from Step Crafter's public description, never from its code (docs/modpacks.md,
//! "Licences"). The mechanics are the port's reading of that description; README.md lists what a
//! harness against the NeoForge original still has to confirm.
//!
//! The block stands on the vanilla dropper's template (its six-way `facing` and `triggered`) with
//! a block entity of type `stepcrafter:step_crafter`. Its nine slots are a 3x3 crafting grid in
//! vanilla's 3x3 menu; hoppers fill them from every side. On each rising redstone edge it matches
//! the grid against `minecraft:crafting` recipes through the engine, takes one item from every
//! occupied slot (leaving each ingredient's crafting remainder, a bucket's bucket), and pushes the
//! result into the storage it faces, or drops it in front when nothing there takes it.
use std::sync::OnceLock;

use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockKey, BlockPart, Compound, Direction, Host, HostTransaction, Hooks,
    Identifier, ItemResource, ItemStack, LevelSide, ModContainer, RecipeInput, Registrar,
    RegistrationContext, ServerHost, Tag, ITEMS,
};

/// The grid's slots.
pub const GRID: usize = 9;

/// The powered state the block last saw, saved as `powered`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Trigger {
    pub powered: bool,
}

impl Trigger {
    /// Sees the block's power this tick: whether to craft (a rising edge).
    pub fn step(&mut self, powered: bool) -> bool {
        let rising = powered && !self.powered;
        self.powered = powered;
        rising
    }
}

/// Takes one item from each occupied grid slot. A slot that empties gets its remainder; a
/// remainder for a slot still holding items comes back in the spill list, to drop.
pub fn consume(grid: &mut [Option<ItemStack>], remainders: Vec<Option<ItemStack>>) -> Vec<ItemStack> {
    let mut spill = Vec::new();
    for (slot, remainder) in grid.iter_mut().zip(remainders.into_iter().chain(std::iter::repeat(None))) {
        let Some(stack) = slot.take() else {
            if let Some(r) = remainder {
                spill.push(r);
            }
            continue;
        };
        *slot = stack.shrink(1);
        if let Some(r) = remainder {
            if slot.is_none() {
                *slot = Some(r);
            } else {
                spill.push(r);
            }
        }
    }
    spill
}

static BLOCK: OnceLock<BlockKey> = OnceLock::new();

fn direction_named(name: &str) -> Option<Direction> {
    Some(match name {
        "north" => Direction::North,
        "south" => Direction::South,
        "east" => Direction::East,
        "west" => Direction::West,
        "up" => Direction::Up,
        "down" => Direction::Down,
        _ => return None,
    })
}

fn normal(dir: Direction) -> [i32; 3] {
    match dir {
        Direction::Down => [0, -1, 0],
        Direction::Up => [0, 1, 0],
        Direction::North => [0, 0, -1],
        Direction::South => [0, 0, 1],
        Direction::West => [-1, 0, 0],
        Direction::East => [1, 0, 0],
    }
}

struct Crafter;

#[mod_api::horizon_part]
impl BlockPart for Crafter {}

#[mod_api::horizon_part]
impl BlockEntityPart for Crafter {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == "stepcrafter:step_crafter").then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == "stepcrafter:step_crafter" {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        if BLOCK.get().is_none_or(|b| level.block(&*host, pos) != Some(*b)) {
            return;
        }
        let mut trigger = Trigger {
            powered: matches!(event.entity.nbt.get(&"powered".into()), Some(Tag::Byte(1))),
        };
        let was = trigger;
        let craft = trigger.step(level.has_neighbor_signal(&*host, pos));
        if trigger != was {
            event
                .entity
                .nbt
                .insert("powered".into(), Tag::Byte(i8::from(trigger.powered)));
            host.set_block_property(pos, "triggered", if trigger.powered { "true" } else { "false" });
        }
        if !craft {
            if trigger == was {
                *event.reply = BlockEntityTickReply::Unchanged;
            }
            return;
        }
        if event.entity.items.len() < GRID {
            event.entity.items.resize(GRID, None);
        }
        let slots: Vec<Option<ItemStack>> = event.entity.items[..GRID].to_vec();
        let Ok(kind) = Identifier::parse("minecraft:crafting") else {
            return;
        };
        let input = RecipeInput::Crafting {
            width: 3,
            height: 3,
            slots,
        };
        let Some(found) = level.recipe_for(&*host, &kind, &input).ok().flatten() else {
            return;
        };
        let facing = level
            .block_property(&*host, pos, "facing")
            .and_then(|f| direction_named(&f))
            .unwrap_or(Direction::North);
        let n = normal(facing);
        let front = [pos[0] + n[0], pos[1] + n[1], pos[2] + n[2]];

        let spill = consume(&mut event.entity.items[..GRID], found.remainders);
        let mut result = Some(found.output);
        if let (Some(out), Some(cap)) = (
            result.clone(),
            level.capability(host, &ITEMS, front, Some(facing.opposite())),
        ) {
            let resource = ItemResource::of(&out);
            let went = host
                .transaction(|host, tx| Ok::<u32, ()>(cap.insert_any(host, tx, &resource, out.count())))
                .unwrap_or(0);
            result = out.shrink(went);
        }
        let at = [
            f64::from(front[0]) + 0.5,
            f64::from(front[1]) + 0.5,
            f64::from(front[2]) + 0.5,
        ];
        for stack in result.into_iter().chain(spill) {
            let _ = level.spawn_item(host, at[0], at[1], at[2], stack, 10);
        }
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let block = r.block_key("stepcrafter:step_crafter");
        let grid: Vec<usize> = (0..GRID).collect();
        r.block_container(
            &block,
            ModContainer {
                slots: GRID,
                faces: std::array::from_fn(|_| grid.clone()),
                inputs: grid.clone(),
                outputs: Vec::new(),
                menu: Some("minecraft:generic_3x3".into()),
                slot_limit: 0,
            },
        );
        let _ = BLOCK.set(block);
        let kind = r
            .block_entity_type("step_crafter")
            .blocks(&["stepcrafter:step_crafter"])
            .build();
        let _ = r.block_entity_part(&kind, Crafter);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_crafts_once_per_rising_edge() {
        let mut t = Trigger::default();
        let crafts: Vec<bool> = [false, true, true, true, false, true, false]
            .into_iter()
            .map(|p| t.step(p))
            .collect();
        assert_eq!(crafts, [false, true, false, false, false, true, false]);
    }

    fn items() -> (mod_api::testing::FakeHost, impl Fn(&mod_api::testing::FakeHost, &str, u32) -> ItemStack) {
        let mut host = mod_api::testing::FakeHost::new();
        let ids: Vec<Identifier> = ["minecraft:oak_planks", "minecraft:milk_bucket", "minecraft:bucket"]
            .iter()
            .map(|i| Identifier::parse(i).unwrap())
            .collect();
        host.registries.resolve_items(ids).unwrap();
        let make = |host: &mod_api::testing::FakeHost, id: &str, n: u32| {
            use mod_api::prelude::ItemsHost;
            ItemStack::new(host.item_key(&Identifier::parse(id).unwrap()).unwrap(), n).unwrap()
        };
        (host, make)
    }

    #[test]
    fn crafting_takes_one_of_each_and_hands_back_remainders() {
        let (host, make) = items();
        let bucket = make(&host, "minecraft:bucket", 1);
        let mut grid = vec![Some(make(&host, "minecraft:oak_planks", 3)), Some(make(&host, "minecraft:milk_bucket", 1)), None];
        let spill = consume(&mut grid, vec![None, Some(bucket.clone()), None]);
        assert_eq!(grid[0].as_ref().map(|s| s.count()), Some(2));
        // the milk bucket emptied, so the empty bucket takes its slot
        assert_eq!(grid[1], Some(bucket));
        assert!(grid[2].is_none());
        assert!(spill.is_empty());
    }

    #[test]
    fn a_remainder_for_a_slot_that_still_holds_items_is_spilled() {
        let (host, make) = items();
        let bucket = make(&host, "minecraft:bucket", 1);
        let mut grid = vec![Some(make(&host, "minecraft:milk_bucket", 2))];
        let spill = consume(&mut grid, vec![Some(bucket.clone())]);
        assert_eq!(grid[0].as_ref().map(|s| s.count()), Some(1));
        assert_eq!(spill, vec![bucket]);
    }
}
