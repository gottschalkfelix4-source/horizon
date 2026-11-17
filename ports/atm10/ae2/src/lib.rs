//! Applied Energistics 2 for Horizon, stage 1: the ME network's blocks (cables in 17 colours,
//! controller), its power (energy acceptor, energy cells) and the network model itself.
//!
//! Written from AE2's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). README.md lists what is done and what a check against the
//! NeoForge original still has to confirm.
//!
//! A network is every AE block joined by faces (see `rules::connects`). Its lowest block runs it each
//! tick: energy acceptors hand FE to the cells, the idle draw of cables and the controller is taken
//! from the cells, and the result (`powered`, `nodes`, `stored`) is saved in that block's data.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, InteractionResult, ItemStack, MenuClickedEvent, MenuClosedEvent, UseItemOnEvent, UseWithoutItemEvent, MenuContainer, MenuPart, MenuSlotRulesEvent, BlockEntityTickEvent, BlockEntityTickReply,
    BlockEntityTicksEvent, BlockPart, Compound, Direction, ENERGY, Hooks, Host, LevelRef,
    LevelSide, HostTransaction, ModContainer, ModEnergyBuffer, Registrar, RegistrationContext, ServerHost, Tag,
};

pub mod autocraft;
pub mod bus;
pub mod cells;
pub mod crafting;
pub mod interface;
pub mod search;
pub mod terminal;
pub mod view;
pub mod wireless;
pub mod rules;
use cells::*;
use rules::*;

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

fn set_int(nbt: &mut Compound, key: &str, v: i32) {
    nbt.insert(key.into(), Tag::Int(v));
}

pub(crate) fn offset(pos: [i32; 3], dir: Direction) -> [i32; 3] {
    let n = dir.normal();
    [pos[0] + n[0], pos[1] + n[1], pos[2] + n[2]]
}

/// The devices of `nodes` that have a channel.
pub(crate) fn channels(host: &dyn Host, level: LevelRef, nodes: &[[i32; 3]]) -> Vec<[i32; 3]> {
    let (mut users, mut controller) = (Vec::new(), false);
    for &n in nodes {
        match id_at(host, level, n).as_deref().and_then(parse_block) {
            Some(Kind::Device(Device::Controller)) => controller = true,
            Some(Kind::Device(d)) if d.needs_channel() => users.push(n),
            _ => {}
        }
    }
    channelled(users, channel_capacity(controller))
}

fn kind_at_pos(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<Kind> {
    id_at(host, level, pos).as_deref().and_then(parse_block)
}

/// The inventories (item capabilities of blocks outside the network) a storage bus at `pos` reaches.
pub(crate) fn bus_inventories(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
) -> Vec<mod_api::Capability<dyn mod_api::ResourceHandler<mod_api::ItemResource>>> {
    let mut out = Vec::new();
    for dir in Direction::ALL {
        let other = offset(pos, dir);
        if id_at(&*host, level, other).as_deref().and_then(parse_block).is_some() {
            continue;
        }
        if let Some(cap) = level.capability(host, &mod_api::ITEMS, other, Some(dir.opposite())) {
            out.push(cap);
        }
    }
    out
}

fn is_storage_bus(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> bool {
    matches!(kind_at_pos(host, level, pos), Some(Kind::Device(Device::StorageBus)))
}

/// What the inventories behind the storage buses of `drives` hold (plain stacks only).
pub(crate) fn inventory_stock(host: &mut dyn ServerHost, level: LevelRef, drives: &[[i32; 3]]) -> Vec<(String, u64)> {
    let mut all: Vec<(String, u64)> = Vec::new();
    for &d in drives {
        if !is_storage_bus(&*host, level, d) {
            continue;
        }
        for cap in bus_inventories(host, level, d) {
            for i in 0..cap.size(host) {
                let Some(res) = cap.resource(host, i) else { continue };
                let Some(stack) = res.to_stack(1) else { continue };
                if stack.components().is_some_and(|c| !c.as_nbt().is_empty()) {
                    continue;
                }
                let id = stack.item().id(&*host);
                let n = cap.amount(host, i) as u64;
                match all.iter_mut().find(|(k, _)| *k == id) {
                    Some((_, c)) => *c += n,
                    None => all.push((id, n)),
                }
            }
        }
    }
    all
}

pub(crate) fn id_at(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<String> {
    Some(level.block(host, pos)?.id(host))
}

/// Moves up to `budget` FE from `from`'s energy to `to`'s in one transaction; how much moved.
fn route(
    host: &mut dyn ServerHost,
    level: LevelRef,
    from: [i32; 3],
    to: [i32; 3],
    budget: u32,
) -> u32 {
    let (Some(src), Some(dst)) = (
        level.capability(host, &ENERGY, from, Some(Direction::Up)),
        level.capability(host, &ENERGY, to, Some(Direction::Down)),
    ) else {
        return 0;
    };
    // what the source can give, found in a transaction that is thrown away
    let held = host
        .transaction(|host, tx| {
            let have = src.amount(host).min(budget);
            let n = src.extract(host, tx, have);
            Err::<(), u32>(n)
        })
        .err()
        .unwrap_or(0);
    if held == 0 {
        return 0;
    }
    host.transaction(|host, tx| {
        let taken = dst.insert(host, tx, held);
        if taken == 0 {
            return Ok(0);
        }
        if src.extract(host, tx, taken) < taken {
            return Err(());
        }
        Ok::<u32, ()>(taken)
    })
    .unwrap_or(0)
}

/// Takes `fe` from the cells, all or nothing.
pub(crate) fn draw(host: &mut dyn ServerHost, level: LevelRef, cells: &[[i32; 3]], fe: u32) -> bool {
    if fe == 0 {
        return true;
    }
    host.transaction(|host, tx| {
        let mut left = fe;
        for &c in cells {
            if left == 0 {
                break;
            }
            let Some(cap) = level.capability(host, &ENERGY, c, Some(Direction::Up)) else {
                continue;
            };
            let have = cap.amount(host).min(left);
            left -= cap.extract(host, tx, have);
        }
        if left == 0 { Ok(()) } else { Err(()) }
    })
    .is_ok()
}

fn stored(host: &mut dyn ServerHost, level: LevelRef, cells: &[[i32; 3]]) -> i64 {
    let mut sum = 0i64;
    for &c in cells {
        if let Some(cap) = level.capability(host, &ENERGY, c, Some(Direction::Up)) {
            sum += cap.amount(host) as i64;
        }
    }
    sum
}

/// The cells in a network's drives: (drive position, slot, tier, stack).
pub(crate) fn cells_of(
    host: &dyn Host,
    level: LevelRef,
    drives: &[[i32; 3]],
) -> Vec<([i32; 3], u32, CellTier, ItemStack)> {
    let mut out = Vec::new();
    for &d in drives {
        let Some(be) = host.block_entity(level, d) else { continue };
        for slot in 0..DRIVE_CELLS as u32 {
            let Ok(Some(stack)) = be.item_slot(host, slot) else { continue };
            if let Some(tier) = parse_cell(&stack.item().id(host)) {
                out.push((d, slot, tier, stack));
            }
        }
    }
    out
}

/// Puts up to `n` of `item` into the network's cells (the first with room, in drive order);
/// how many went in. Stage 3's terminal and the buses call this.
pub fn network_insert(
    host: &mut dyn ServerHost,
    level: LevelRef,
    drives: &[[i32; 3]],
    item: &str,
    n: u64,
) -> u64 {
    let cells = cells_of(&*host, level, drives);
    let mut left = n;
    let _ = host.transaction(|host, tx| {
        for (d, slot, tier, mut stack) in cells {
            if left == 0 {
                break;
            }
            let mut store = Store::of_stack(&stack);
            let put = store.insert(tier, item, left);
            if put == 0 {
                continue;
            }
            store.write_to(&mut stack);
            let be = host.block_entity(level, d).ok_or(())?;
            be.set_item_slot(host, tx, slot, Some(stack)).map_err(|_| ())?;
            left -= put;
        }
        Ok::<(), ()>(())
    });
    // then the inventories behind storage buses
    if left > 0 && plain_item(&*host, item) {
        'buses: for &d in drives {
            if !is_storage_bus(&*host, level, d) {
                continue;
            }
            let Some(res) = bus::resource_of(&*host, item) else { break };
            for cap in bus_inventories(host, level, d) {
                let put = host
                    .transaction(|host, tx| Ok::<u32, ()>(cap.insert_any(host, tx, &res, left.min(u32::MAX as u64) as u32)))
                    .unwrap_or(0);
                left -= put as u64;
                if left == 0 {
                    break 'buses;
                }
            }
        }
    }
    n - left
}

fn plain_item(host: &dyn Host, id: &str) -> bool {
    mod_api::Identifier::parse(id).ok().and_then(|i| host.item_key(&i)).is_some()
}

/// Takes up to `n` of `item` out of the network's cells; how many came out.
pub fn network_extract(
    host: &mut dyn ServerHost,
    level: LevelRef,
    drives: &[[i32; 3]],
    item: &str,
    n: u64,
) -> u64 {
    let cells = cells_of(&*host, level, drives);
    let mut left = n;
    let _ = host.transaction(|host, tx| {
        for (d, slot, _, mut stack) in cells {
            if left == 0 {
                break;
            }
            let mut store = Store::of_stack(&stack);
            let got = store.extract(item, left);
            if got == 0 {
                continue;
            }
            store.write_to(&mut stack);
            let be = host.block_entity(level, d).ok_or(())?;
            be.set_item_slot(host, tx, slot, Some(stack)).map_err(|_| ())?;
            left -= got;
        }
        Ok::<(), ()>(())
    });
    if left > 0 {
        'buses: for &d in drives {
            if !is_storage_bus(&*host, level, d) {
                continue;
            }
            let Some(res) = bus::resource_of(&*host, item) else { break };
            for cap in bus_inventories(host, level, d) {
                let got = host
                    .transaction(|host, tx| Ok::<u32, ()>(cap.extract_any(host, tx, &res, left.min(u32::MAX as u64) as u32)))
                    .unwrap_or(0);
                left -= got as u64;
                if left == 0 {
                    break 'buses;
                }
            }
        }
    }
    n - left
}

/// One tick of the network `pos` leads; returns what the leader saves.
fn tick_network(
    host: &mut dyn ServerHost,
    level: LevelRef,
    pos: [i32; 3],
    carried: i64,
) -> Option<(usize, bool, i64, i64, [i32; 4])> {
    let class = |host: &dyn Host, p: [i32; 3]| {
        id_at(host, level, p).as_deref().and_then(parse_block).map_or(2, lead_class)
    };
    let keyed = led_network((class(&*host, pos), pos), |(_, p)| {
        let Some(here) = id_at(&*host, level, p) else { return Vec::new() };
        Direction::ALL
            .into_iter()
            .map(|d| offset(p, d))
            .filter(|n| id_at(&*host, level, *n).is_some_and(|o| connects(&here, &o)))
            .map(|n| (class(&*host, n), n))
            .collect()
    })?;
    let nodes: Vec<[i32; 3]> = keyed.into_iter().map(|(_, p)| p).collect();
    let has_channel = channels(&*host, level, &nodes);
    let (mut cells, mut acceptors, mut drives, mut idle) = (Vec::new(), Vec::new(), Vec::new(), 0i64);
    for &n in &nodes {
        match id_at(&*host, level, n).as_deref().and_then(parse_block) {
            Some(Kind::Device(Device::EnergyAcceptor)) => acceptors.push(n),
            Some(Kind::Device(Device::Drive)) => {
                if has_channel.contains(&n) {
                    drives.push(n);
                }
            }
            Some(Kind::Device(d)) if d.is_cell() => cells.push(n),
            Some(k) => idle += k.idle_centi_fe(),
            None => {}
        }
    }
    // (a network that has a cell or acceptor as its leader skips that block's own reads)
    // what the acceptors took in goes into the cells
    for &a in &acceptors {
        for &c in &cells {
            route(host, level, a, c, u32::MAX);
        }
    }
    let (take, rem) = drain(idle, carried);
    let ok = powered(stored(host, level, &cells), take) && draw(host, level, &cells, take as u32);
    // an unpowered network keeps the carried hundredths; nothing was taken
    let rem = if ok { rem } else { carried };
    // what the drives' cells hold: types, items, bytes used and bytes in all
    let (mut types, mut items, mut used, mut bytes) = (0u64, 0u64, 0u64, 0u64);
    for (_, _, tier, stack) in cells_of(&*host, level, &drives) {
        let s = Store::of_stack(&stack);
        types += s.types() as u64;
        items += s.total();
        used += s.used_bytes(tier);
        bytes += tier.bytes();
    }
    let c = |v: u64| v.min(i32::MAX as u64) as i32;
    Some((nodes.len(), ok, stored(host, level, &cells), rem, [c(types), c(items), c(used), c(bytes)]))
}

/// One part for every AE2 block entity; the block's id says which kind it is.
struct Network;

#[mod_api::horizon_part]
impl BlockPart for Network {}

#[mod_api::horizon_part]
impl BlockEntityPart for Network {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        parse_block(event.block).map(|_| {
            let mut nbt = Compound::new();
            set_int(&mut nbt, "energy", 0);
            nbt
        })
    }

    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if parse_block(event.block).is_some() {
            event.ticks = true;
        }
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        let Some(level) = host.level(host.dimension()) else { return };
        if let Some(Kind::Device(d)) = kind_at_pos(&*host, level, pos).filter(|k| matches!(k, Kind::Device(d) if d.is_bus())) {
            // the filter is the item in the bus's first slot (else a filter saved by `merge_data`)
            let slot_filter = event.entity.items.first().cloned().flatten().map(|s| s.item().id(&*host));
            bus::tick(host, level, pos, d, slot_filter.or_else(|| bus::filter_of(&event.entity.nbt)));
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        }
        let carried = int(&event.entity.nbt, "drain_rem") as i64;
        let Some((nodes, powered, stored, rem, storage)) = tick_network(host, level, pos, carried) else {
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        };
        let mut nbt = event.entity.nbt.clone();
        set_int(&mut nbt, "nodes", nodes as i32);
        set_int(&mut nbt, "powered", i32::from(powered));
        set_int(&mut nbt, "stored", stored.min(i32::MAX as i64) as i32);
        set_int(&mut nbt, "drain_rem", rem as i32);
        for (k, v) in ["types", "items", "bytes_used", "bytes_total"].into_iter().zip(storage) {
            set_int(&mut nbt, k, v);
        }
        if nbt == event.entity.nbt {
            *event.reply = BlockEntityTickReply::Unchanged;
            return;
        }
        event.entity.nbt = nbt;
    }
}

/// The game's energy buffer for a block: cells and the acceptor move FE through every face
/// (the acceptor only takes it in from outside; the network hands it on to the cells).
fn buffer_of(d: Device) -> Option<ModEnergyBuffer> {
    let cap = d.capacity_fe();
    (cap > 0).then(|| ModEnergyBuffer {
        capacity: cap,
        max_insert: cap,
        max_extract: cap,
        insert_faces: [true; 6],
        extract_faces: [true; 6],
    })
}

/// The ME terminal block: using it opens the network's items.
struct Terminal;

#[mod_api::horizon_part]
impl BlockPart for Terminal {
    fn use_item_on(&self, host: &mut dyn ServerHost, event: &mut UseItemOnEvent<'_>) -> Option<InteractionResult> {
        terminal::open(host, event.click.player, event.click.pos)
    }

    fn use_without_item(&self, host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
        terminal::open(host, event.click.player, event.click.pos)
    }
}

/// The drive takes only storage cells, in its first ten slots.
struct DriveRules;

#[mod_api::horizon_part]
impl MenuPart for DriveRules {
    fn menu_slot_rules(&self, host: &dyn Host, event: &mut MenuSlotRulesEvent<'_>) {
        if event.container != MenuContainer::Main {
            return;
        }
        let Some(pos) = event.context.pos else { return };
        let Some(level) = host.level(host.dimension()) else { return };
        if !level.block(host, pos).is_some_and(|b| b.id(host) == "ae2:drive") {
            return;
        }
        if event.slot >= DRIVE_CELLS || parse_cell(&event.stack.item().id(host)).is_none() {
            event.may_place = false;
        }
    }
}

/// The pattern provider takes only encoded patterns.
struct ProviderRules;

#[mod_api::horizon_part]
impl MenuPart for ProviderRules {
    fn menu_slot_rules(&self, host: &dyn Host, event: &mut MenuSlotRulesEvent<'_>) {
        if event.container != MenuContainer::Main {
            return;
        }
        let Some(pos) = event.context.pos else { return };
        let Some(level) = host.level(host.dimension()) else { return };
        let Some(id) = level.block(host, pos).map(|b| b.id(host)) else { return };
        match id.as_str() {
            "ae2:pattern_provider" => {
                if event.slot >= 9 || event.stack.item().id(host) != autocraft::PATTERN {
                    event.may_place = false;
                }
            }
            // a bus keeps its filter in slot 0: the item put there is the filter
            "ae2:import_bus" | "ae2:export_bus" => {
                if event.slot != 0 {
                    event.may_place = false;
                }
            }
            _ => {}
        }
    }
}

/// The drive: ten cell slots. Vanilla's 9x2 menu has 18, so the drive keeps 18 and only the first
/// ten count (until the mod's own menu with ten slots exists).
fn drive_container() -> ModContainer {
    let all: Vec<usize> = (0..18).collect();
    ModContainer {
        slots: 18,
        faces: std::array::from_fn(|_| Vec::new()),
        inputs: all.clone(),
        outputs: all,
        menu: Some("minecraft:generic_9x2".into()),
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let mut groups: Vec<(String, Vec<String>)> = Vec::new();
        for c in Cable::ALL {
            groups.push((c.name().into(), COLORS.iter().map(|col| cable_id(col, c)).collect()));
        }
        for d in Device::ALL {
            groups.push((d.name().into(), vec![device_id(d)]));
        }
        for (entity_type, ids) in groups {
            for id in &ids {
                if let Some(Kind::Device(d)) = parse_block(id) {
                    if let Some(buf) = buffer_of(d) {
                        let key = r.block_key(id);
                        r.block_energy(&key, buf);
                    }
                }
            }
            let names: Vec<&str> = ids.iter().map(String::as_str).collect();
            if entity_type == "import_bus" || entity_type == "export_bus" {
                // the filter slot: open the bus, put the item to filter by in the first slot
                for id in &ids {
                    let key = r.block_key(id);
                    let all: Vec<usize> = (0..9).collect();
                    r.block_container(
                        &key,
                        ModContainer {
                            slots: 9,
                            faces: std::array::from_fn(|_| Vec::new()),
                            inputs: all.clone(),
                            outputs: all,
                            menu: Some("minecraft:generic_9x1".into()),
                        },
                    );
                }
            }
            if entity_type == "interface" {
                let built = r.block_entity_type("interface").blocks(&["ae2:interface"]).build();
                let _ = r.block_entity_part(&built, Network);
                r.block_entity_capability(&mod_api::ITEMS, &built, |_, at: mod_api::BlockSide| {
                    Some(Box::new(interface::Interface { level: at.level, pos: at.pos })
                        as Box<dyn mod_api::ResourceHandler<mod_api::ItemResource>>)
                });
                continue;
            }
            if entity_type == "terminal" {
                let key = r.block_key("ae2:terminal");
                r.block_part(key, Terminal);
            }
            if entity_type == "pattern_provider" {
                let key = r.block_key("ae2:pattern_provider");
                let all: Vec<usize> = (0..9).collect();
                r.block_container(
                    &key,
                    ModContainer {
                        slots: 9,
                        faces: std::array::from_fn(|_| Vec::new()),
                        inputs: all.clone(),
                        outputs: all,
                        menu: Some("minecraft:generic_9x1".into()),
                    },
                );
            }
            if entity_type == "drive" {
                let key = r.block_key("ae2:drive");
                r.block_container(&key, drive_container());
                r.menu_part("minecraft:generic_9x2", DriveRules);
            }
            let built = r.block_entity_type(&entity_type).blocks(&names).build();
            let _ = r.block_entity_part(&built, Network);
        }
        let mut handlers = r.payload_handlers();
        let mut payloads = handlers.registrar("1");
        let token = payloads.play_to_server_typed::<String>(search::id(), |text, context| {
            let player = context.player().map_err(|e| format!("{e:?}"))?;
            terminal::queue_search(player.uuid(), text);
            Ok(())
        });
        let _ = search::SEARCH.set(token);
        let wt = r.item_key(wireless::ITEM);
        r.item_part(wt, wireless::Terminal);
        // one rule for the 9x1 menu: pattern provider and both buses (the block says which)
        r.menu_part("minecraft:generic_9x1", ProviderRules);
    }

    fn server_tick(&self, host: &mut dyn ServerHost, _: &mut mod_api::ServerTickEvent<'_>) {
        terminal::apply_searches(host);
        autocraft::tick(host);
    }

    fn menu_clicked(&self, host: &mut dyn ServerHost, event: &mut MenuClickedEvent) {
        terminal::clicked(host, event);
    }

    fn menu_closed(&self, _: &mut dyn ServerHost, event: &mut MenuClosedEvent) {
        terminal::closed(event.uuid, event.window);
    }

    fn screen_key_pressed_pre(&self, _: &dyn mod_api::ClientHost, event: &mut mod_api::ScreenKeyPressedPreEvent<'_>) -> mod_api::Flow {
        search::key_pressed(event)
    }

    fn screen_char_typed_pre(&self, _: &dyn mod_api::ClientHost, event: &mut mod_api::ScreenCharTypedPreEvent<'_>) -> mod_api::Flow {
        search::char_typed(event)
    }

    fn client_tick_post(&self, host: &mut dyn mod_api::ClientHost, event: &mut mod_api::ClientTickPostEvent) {
        search::send(host, event);
    }

    fn screen_closing(&self, host: &dyn mod_api::ClientHost, event: &mut mod_api::ScreenClosingEvent<'_>) {
        search::closing(host, event);
    }
}
