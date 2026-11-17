//! Modular Routers for Horizon: a router block that runs the modules put in it, each every few
//! ticks, on the one-slot buffer it holds.
//!
//! Written from Modular Routers' observed behaviour and public description (its in-game manual and
//! its CurseForge and Modrinth pages), never from its code (docs/modpacks.md, "Licences"). What the
//! port does, and what a harness against the NeoForge original still has to confirm, is in
//! README.md.
//!
//! The router is a block on the vanilla furnace's template (for its `facing`), with a block entity
//! of type `modularrouters:modular_router`. Its 18 slots open vanilla's 9x2 chest menu: slot 0 is
//! the buffer, slots 1 to 9 the modules, slots 10 to 14 the upgrades. Hoppers and pipes reach the
//! buffer from every side. Every `tick_rate` ticks the router runs each module in slot order: a
//! sender pushes the buffer into the inventory it faces, a puller pulls from it, a dropper drops
//! items, a placer places the buffer's block, a breaker breaks the block it faces into the buffer,
//! a vacuum picks up item entities around the router, a void module deletes the buffer.
use std::sync::OnceLock;

use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockEntityTickEvent, BlockEntityTicksEvent, BlockKey,
    BlockPart, BlockUpdateFlags, Compound, Direction, HostBlockStates, HostSide, HostTransaction, Hooks, Host,
    InteractionResult, ItemEntityRef, ItemKey, ItemPart, ItemResource, ItemStack, LevelRef,
    LevelSide, ModContainer, PlayerRef, Registrar, RegistrationContext, ServerHost, Tag,
    UseItemEvent, ITEMS,
};

/// Slots, as the router's 9x2 menu numbers them.
pub const BUFFER: usize = 0;
/// The nine module slots.
pub const MODULES: std::ops::RangeInclusive<usize> = 1..=9;
/// The five upgrade slots.
pub const UPGRADES: std::ops::RangeInclusive<usize> = 10..=14;
/// Slots in the router's container (the 9x2 menu's).
pub const SLOTS: usize = 18;

/// Ticks between two runs with no speed upgrade.
pub const BASE_TICK_RATE: u32 = 20;
/// Ticks one speed upgrade takes off.
pub const SPEED_STEP: u32 = 2;
/// The fastest a router runs.
pub const MIN_TICK_RATE: u32 = 2;

/// What the installed upgrades make of a router.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    /// Ticks between two runs of the modules.
    pub tick_rate: u32,
    /// Items one module operation moves.
    pub items_per_op: u32,
    /// Blocks added to the range of ranged modules.
    pub range_bonus: i32,
}

impl Stats {
    /// From the number of speed, stack and range upgrades installed.
    pub fn from_upgrades(speed: u32, stack: u32, range: u32) -> Self {
        let tick_rate = BASE_TICK_RATE
            .saturating_sub(SPEED_STEP.saturating_mul(speed))
            .max(MIN_TICK_RATE);
        let items_per_op = 1u32 << stack.min(6);
        Self {
            tick_rate,
            items_per_op,
            range_bonus: range.min(64) as i32,
        }
    }
}

/// The modules this port runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Module {
    Sender1,
    Sender2,
    Puller1,
    Dropper,
    Placer,
    Breaker,
    Vacuum,
    Void,
}

impl Module {
    pub const ALL: [Module; 8] = [
        Module::Sender1,
        Module::Sender2,
        Module::Puller1,
        Module::Dropper,
        Module::Placer,
        Module::Breaker,
        Module::Vacuum,
        Module::Void,
    ];

    /// The item's path in the `modularrouters` namespace.
    pub fn name(self) -> &'static str {
        match self {
            Module::Sender1 => "sender_module_1",
            Module::Sender2 => "sender_module_2",
            Module::Puller1 => "puller_module_1",
            Module::Dropper => "dropper_module",
            Module::Placer => "placer_module",
            Module::Breaker => "breaker_module",
            Module::Vacuum => "vacuum_module",
            Module::Void => "void_module",
        }
    }

    /// The module's range with no range upgrade, and the most range upgrades can give it; none
    /// for a module that acts only on the block beside it.
    pub fn range(self) -> Option<(i32, i32)> {
        match self {
            Module::Sender2 => Some((8, 16)),
            Module::Vacuum => Some((6, 12)),
            _ => None,
        }
    }

    /// The range the module reaches with `bonus` blocks from range upgrades.
    pub fn effective_range(self, bonus: i32) -> i32 {
        self.range()
            .map_or(1, |(base, max)| (base + bonus).clamp(1, max))
    }

    /// Whether the module looks at a direction (a module with none acts on the router itself).
    pub fn directional(self) -> bool {
        !matches!(self, Module::Vacuum | Module::Void)
    }
}

/// A module's direction, relative to the router's front (the module's GUI setting).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relative {
    Front,
    Back,
    Up,
    Down,
    Left,
    Right,
}

impl Relative {
    pub const CYCLE: [Relative; 6] = [
        Relative::Front,
        Relative::Back,
        Relative::Up,
        Relative::Down,
        Relative::Left,
        Relative::Right,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Relative::Front => "front",
            Relative::Back => "back",
            Relative::Up => "up",
            Relative::Down => "down",
            Relative::Left => "left",
            Relative::Right => "right",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::CYCLE.into_iter().find(|r| r.name() == name)
    }

    /// The next setting, as clicking the direction button does.
    pub fn next(self) -> Self {
        let i = Self::CYCLE.iter().position(|r| *r == self).unwrap_or(0);
        Self::CYCLE[(i + 1) % Self::CYCLE.len()]
    }

    /// The world direction for a router whose front faces `front` (a horizontal direction).
    /// Left and right are as seen by someone standing in front of the router, facing it.
    pub fn absolute(self, front: Direction) -> Direction {
        match self {
            Relative::Front => front,
            Relative::Back => front.opposite(),
            Relative::Up => Direction::Up,
            Relative::Down => Direction::Down,
            Relative::Left => clockwise(front),
            Relative::Right => clockwise(front).opposite(),
        }
    }
}

/// `dir` turned a quarter clockwise, seen from above; up and down stay.
fn clockwise(dir: Direction) -> Direction {
    match dir {
        Direction::North => Direction::East,
        Direction::East => Direction::South,
        Direction::South => Direction::West,
        Direction::West => Direction::North,
        other => other,
    }
}

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

fn offset(pos: [i32; 3], dir: Direction, by: i32) -> [i32; 3] {
    let n = normal(dir);
    [pos[0] + n[0] * by, pos[1] + n[1] * by, pos[2] + n[2] * by]
}

/// How many of `count` items fit on a stack of `held` (of the same kind) up to `max`.
pub fn room(held: u32, max: u32) -> u32 {
    max.saturating_sub(held)
}

/// A module stack's direction setting, saved in its `minecraft:custom_data` as `direction`.
pub fn module_direction(stack: &ItemStack) -> Relative {
    custom_string(stack, "direction")
        .and_then(|s| Relative::parse(&s))
        .unwrap_or(Relative::Front)
}

fn custom_string(stack: &ItemStack, key: &str) -> Option<String> {
    let patch = stack.components()?.as_nbt();
    let Some(Tag::Compound(data)) = patch.get(&"minecraft:custom_data".into()) else {
        return None;
    };
    match data.get(&key.into()) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    }
}

fn with_custom_string(stack: &mut ItemStack, key: &str, value: &str) {
    let mut data = stack
        .components()
        .and_then(|c| match c.as_nbt().get(&"minecraft:custom_data".into()) {
            Some(Tag::Compound(d)) => Some(d.clone()),
            _ => None,
        })
        .unwrap_or_default();
    data.insert(key.into(), Tag::String(value.into()));
    let mut patch = Compound::new();
    patch.insert("minecraft:custom_data".into(), Tag::Compound(data));
    let _ = stack.apply_components(&patch);
}

struct Keys {
    router: BlockKey,
    modules: Vec<(ItemKey, Module)>,
    speed: ItemKey,
    stack: ItemKey,
    range: ItemKey,
}

static KEYS: OnceLock<Keys> = OnceLock::new();

fn module_of(item: ItemKey) -> Option<Module> {
    KEYS.get()?
        .modules
        .iter()
        .find_map(|(k, m)| (*k == item).then_some(*m))
}

/// The router's buffer and level, for one run of its modules.
struct Run<'a> {
    level: LevelRef,
    pos: [i32; 3],
    front: Direction,
    stats: Stats,
    buffer: &'a mut Option<ItemStack>,
}

impl Run<'_> {
    fn max_stack(&self, host: &dyn Host, stack: &ItemStack) -> u32 {
        host.max_stack_size(stack.item()).unwrap_or(64)
    }

    fn take_from_buffer(&mut self, count: u32) {
        if let Some(held) = self.buffer.take() {
            *self.buffer = held.shrink(count);
        }
    }

    /// Adds `stack` to the buffer as far as it fits: what is left over.
    fn add_to_buffer(&mut self, host: &dyn Host, stack: ItemStack) -> Option<ItemStack> {
        match self.buffer.take() {
            None => {
                *self.buffer = Some(stack);
                None
            }
            Some(mut held) => {
                if !held.is_stackable_with(&stack) {
                    *self.buffer = Some(held);
                    return Some(stack);
                }
                let fit = room(held.count(), self.max_stack(host, &held)).min(stack.count());
                let _ = held.grow(fit);
                *self.buffer = Some(held);
                stack.shrink(fit)
            }
        }
    }

    /// Pushes up to `items_per_op` of the buffer into the inventory at `target`, reached through
    /// its face `side`.
    fn send(&mut self, host: &mut dyn ServerHost, target: [i32; 3], side: Direction) {
        let Some(held) = self.buffer.clone() else {
            return;
        };
        let Some(cap) = self.level.capability(host, &ITEMS, target, Some(side)) else {
            return;
        };
        let resource = ItemResource::of(&held);
        let want = self.stats.items_per_op.min(held.count());
        let went = host
            .transaction(|host, tx| Ok::<u32, ()>(cap.insert_any(host, tx, &resource, want)))
            .unwrap_or(0);
        self.take_from_buffer(went);
    }

    fn pull(&mut self, host: &mut dyn ServerHost, target: [i32; 3], side: Direction) {
        let Some(cap) = self.level.capability(host, &ITEMS, target, Some(side)) else {
            return;
        };
        for index in 0..cap.size(host) {
            let Some(resource) = cap.resource(host, index) else {
                continue;
            };
            let space = match self.buffer.as_ref() {
                None => self.max_stack(host, resource.as_stack()),
                Some(held) if resource.matches(held) => {
                    room(held.count(), self.max_stack(host, held))
                }
                Some(_) => 0,
            };
            let want = space.min(self.stats.items_per_op);
            if want == 0 {
                continue;
            }
            let came = host
                .transaction(|host, tx| Ok::<u32, ()>(cap.extract(host, tx, index, &resource, want)))
                .unwrap_or(0);
            if let Some(stack) = resource.to_stack(came) {
                let _ = self.add_to_buffer(host, stack);
                return;
            }
        }
    }

    fn drop_items(&mut self, host: &mut dyn ServerHost, target: [i32; 3]) {
        let Some(held) = self.buffer.clone() else {
            return;
        };
        let n = self.stats.items_per_op.min(held.count());
        let mut out = held;
        if out.set_count(n).is_err() {
            return;
        }
        let at = [f64::from(target[0]) + 0.5, f64::from(target[1]) + 0.5, f64::from(target[2]) + 0.5];
        if matches!(self.level.spawn_item(host, at[0], at[1], at[2], out, 0), Ok(Some(_))) {
            self.take_from_buffer(n);
        }
    }

    fn place(&mut self, host: &mut dyn ServerHost, target: [i32; 3]) {
        let Some(held) = self.buffer.clone() else {
            return;
        };
        let Some(state) = host
            .item_block(held.item())
            .and_then(|b| host.block_state_default(b))
        else {
            return;
        };
        let replaceable = self
            .level
            .block_state(host, target)
            .and_then(|s| host.block_states().replaceable(s))
            .unwrap_or(false);
        if !replaceable {
            return;
        }
        let flags = BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS;
        if matches!(self.level.set_block(host, target, state, flags), Ok(true)) {
            self.take_from_buffer(1);
        }
    }

    fn break_block(&mut self, host: &mut dyn ServerHost, target: [i32; 3]) {
        let Some(state) = self.level.block_state(host, target) else {
            return;
        };
        let unbreakable = host.block_states().destroy_speed(state).is_none_or(|s| s < 0.0);
        let air = host.block_states().replaceable(state).unwrap_or(true);
        if unbreakable || air {
            return;
        }
        let Ok(Some(drops)) = self.level.destroy_block(host, target, false, None, true) else {
            return;
        };
        for drop in drops {
            if let Some(rest) = self.add_to_buffer(host, drop) {
                let at = [f64::from(target[0]) + 0.5, f64::from(target[1]) + 0.5, f64::from(target[2]) + 0.5];
                let _ = self.level.spawn_item(host, at[0], at[1], at[2], rest, 10);
            }
        }
    }

    fn vacuum(&mut self, host: &mut dyn ServerHost, range: i32) {
        let r = f64::from(range);
        let c = [f64::from(self.pos[0]) + 0.5, f64::from(self.pos[1]) + 0.5, f64::from(self.pos[2]) + 0.5];
        let Some(found) = self
            .level
            .entities_in(host, [c[0] - r, c[1] - r, c[2] - r], [c[0] + r + 1.0, c[1] + r + 1.0, c[2] + r + 1.0])
        else {
            return;
        };
        let mut budget = self.stats.items_per_op;
        for entity in found {
            if budget == 0 {
                return;
            }
            let item = ItemEntityRef::new(entity);
            let Some(stack) = item.stack(host) else {
                continue;
            };
            let total = stack.count();
            let take = total.min(budget);
            let Some(mut taken) = stack.clone().shrink(total - take) else {
                continue;
            };
            let _ = taken.set_count(take);
            let rest = self.add_to_buffer(host, taken);
            let kept = rest.map_or(0, |r| r.count());
            let moved = take - kept;
            if moved == 0 {
                continue;
            }
            budget -= moved;
            match stack.shrink(moved) {
                Some(left) => {
                    let _ = item.set_stack(host, left);
                }
                None => {
                    let _ = entity.discard(host);
                }
            }
        }
    }
}

struct Router;

#[mod_api::horizon_part]
impl BlockPart for Router {}

#[mod_api::horizon_part]
impl BlockEntityPart for Router {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == "modularrouters:modular_router").then(Compound::new)
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if event.block == "modularrouters:modular_router" {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let Some(keys) = KEYS.get() else {
            return;
        };
        let Some(level) = host.level(host.dimension()) else {
            return;
        };
        if level.block(host, pos) != Some(keys.router) {
            return;
        }
        if event.entity.items.len() < SLOTS {
            event.entity.items.resize(SLOTS, None);
        }
        let count_of = |item: ItemKey, items: &[Option<ItemStack>]| -> u32 {
            UPGRADES
                .filter_map(|i| items[i].as_ref())
                .filter(|s| s.item() == item)
                .map(ItemStack::count)
                .sum()
        };
        let stats = Stats::from_upgrades(
            count_of(keys.speed, &event.entity.items),
            count_of(keys.stack, &event.entity.items),
            count_of(keys.range, &event.entity.items),
        );
        let counter = match event.entity.nbt.get(&"counter".into()) {
            Some(Tag::Int(v)) => *v as u32,
            _ => 0,
        } + 1;
        if counter < stats.tick_rate {
            event.entity.nbt.insert("counter".into(), Tag::Int(counter as i32));
            return;
        }
        event.entity.nbt.insert("counter".into(), Tag::Int(0));

        let front = level
            .block_property(host, pos, "facing")
            .and_then(|f| direction_named(&f))
            .unwrap_or(Direction::North);
        let modules: Vec<(Module, Relative)> = MODULES
            .filter_map(|i| event.entity.items[i].as_ref())
            .filter_map(|s| Some((module_of(s.item())?, module_direction(s))))
            .collect();
        let mut buffer = event.entity.items[BUFFER].take();
        let mut run = Run {
            level,
            pos,
            front,
            stats,
            buffer: &mut buffer,
        };
        for (module, relative) in modules {
            let dir = relative.absolute(run.front);
            let next = offset(pos, dir, 1);
            match module {
                Module::Sender1 => run.send(host, next, dir.opposite()),
                Module::Sender2 => {
                    // The first inventory along the line, up to the module's range.
                    let range = module.effective_range(stats.range_bonus);
                    if let Some(at) = (1..=range)
                        .map(|d| offset(pos, dir, d))
                        .find(|p| level.capability(host, &ITEMS, *p, Some(dir.opposite())).is_some())
                    {
                        run.send(host, at, dir.opposite());
                    }
                }
                Module::Puller1 => run.pull(host, next, dir.opposite()),
                Module::Dropper => run.drop_items(host, next),
                Module::Placer => run.place(host, next),
                Module::Breaker => run.break_block(host, next),
                Module::Vacuum => run.vacuum(host, module.effective_range(stats.range_bonus)),
                Module::Void => {
                    let n = stats.items_per_op;
                    run.take_from_buffer(n);
                }
            }
        }
        event.entity.items[BUFFER] = buffer;
    }
}

/// A module item: sneak-using it in the air turns its direction one step on (the port's stand-in
/// for the module's settings screen, which needs a mod screen).
struct ModuleItem;

#[mod_api::horizon_part]
impl ItemPart for ModuleItem {
    fn use_item(&self, host: HostSide<'_>, event: &mut UseItemEvent<'_>) -> Option<InteractionResult> {
        let module = module_of(event.stack.item())?;
        if !module.directional() {
            return None;
        }
        let HostSide::Server(host) = host else {
            return None;
        };
        let player: PlayerRef = host.player(event.player)?;
        if player.entity().is_shift_key_down(host) != Some(true) {
            return None;
        }
        let mut stack = event.stack.clone();
        let next = module_direction(&stack).next();
        with_custom_string(&mut stack, "direction", next.name());
        let living = mod_api::LivingRef::new(player.entity());
        let _ = living.set_hand_item(host, event.hand, Some(stack));
        Some(InteractionResult::SuccessServer)
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let router = r.block_key("modularrouters:modular_router");
        let all: Vec<usize> = vec![BUFFER];
        r.block_container(
            &router,
            ModContainer {
                slots: SLOTS,
                faces: [all.clone(), all.clone(), all.clone(), all.clone(), all.clone(), all.clone()],
                inputs: vec![BUFFER],
                outputs: vec![BUFFER],
                menu: Some("minecraft:generic_9x2".into()),
                slot_limit: 0,
            },
        );
        let mut modules = Vec::new();
        for module in Module::ALL {
            let item = r.item_key(&format!("modularrouters:{}", module.name()));
            r.item_part(item, ModuleItem);
            modules.push((item, module));
        }
        let keys = Keys {
            router,
            modules,
            speed: r.item_key("modularrouters:speed_upgrade"),
            stack: r.item_key("modularrouters:stack_upgrade"),
            range: r.item_key("modularrouters:range_upgrade"),
        };
        let _ = KEYS.set(keys);
        let kind = r
            .block_entity_type("modular_router")
            .blocks(&["modularrouters:modular_router"])
            .build();
        let _ = r.block_entity_part(&kind, Router);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_upgrades_take_two_ticks_each_down_to_two() {
        assert_eq!(Stats::from_upgrades(0, 0, 0).tick_rate, 20);
        assert_eq!(Stats::from_upgrades(3, 0, 0).tick_rate, 14);
        assert_eq!(Stats::from_upgrades(9, 0, 0).tick_rate, 2);
        assert_eq!(Stats::from_upgrades(40, 0, 0).tick_rate, 2);
    }

    #[test]
    fn stack_upgrades_double_the_items_per_operation_up_to_a_stack() {
        let per_op: Vec<u32> = (0..8).map(|n| Stats::from_upgrades(0, n, 0).items_per_op).collect();
        assert_eq!(per_op, [1, 2, 4, 8, 16, 32, 64, 64]);
    }

    #[test]
    fn range_upgrades_extend_ranged_modules_up_to_their_cap() {
        assert_eq!(Module::Vacuum.effective_range(0), 6);
        assert_eq!(Module::Vacuum.effective_range(4), 10);
        assert_eq!(Module::Vacuum.effective_range(20), 12);
        assert_eq!(Module::Sender2.effective_range(3), 11);
        assert_eq!(Module::Sender1.effective_range(5), 1);
    }

    #[test]
    fn relative_directions_turn_with_the_router() {
        assert_eq!(Relative::Front.absolute(Direction::North), Direction::North);
        assert_eq!(Relative::Back.absolute(Direction::North), Direction::South);
        assert_eq!(Relative::Left.absolute(Direction::North), Direction::East);
        assert_eq!(Relative::Right.absolute(Direction::North), Direction::West);
        assert_eq!(Relative::Left.absolute(Direction::East), Direction::South);
        assert_eq!(Relative::Up.absolute(Direction::West), Direction::Up);
    }

    #[test]
    fn the_direction_setting_cycles_through_all_six_and_back() {
        let mut r = Relative::Front;
        let mut seen = vec![r];
        for _ in 0..5 {
            r = r.next();
            seen.push(r);
        }
        assert_eq!(seen, Relative::CYCLE);
        assert_eq!(r.next(), Relative::Front);
        assert_eq!(Relative::parse("left"), Some(Relative::Left));
        assert_eq!(Relative::parse("sideways"), None);
    }
}
