//! Refined Storage for Horizon: the storage network. A Controller joins every Storage Block it
//! reaches through Cables (and through the network's own blocks) into one inventory, which
//! counts items rather than stacks: a 1k Storage Block holds a thousand items of any mix.
//!
//! Written from Refined Storage's observed behaviour and public description, never from its code
//! (docs/modpacks.md, "Licences"). What is from memory is listed in README.md.
//!
//! How it works: each Storage Block has a block entity whose saved compound holds what it stores
//! under `stored` (one entry per item type: the item and its amount). The Controller answers the
//! `horizon:items` capability with the whole network: asked for it, it walks the connected
//! blocks, reads every Storage Block's contents, and inserts or extracts across them, writing
//! back only the ones that changed. A hopper, a pipe or another mod's importer on any side of the
//! Controller therefore fills or empties the network.
use std::collections::{HashSet, VecDeque};
use std::sync::OnceLock;

use mod_api::{
    BlockKey, Compound, Hooks, ITEMS, ItemResource, ItemStack, LevelRef, Registrar,
    RegistrationContext, ResourceHandler, ServerHost, Tag, Transaction,
};

/// The Storage Blocks and how many items each holds.
pub const STORAGE_BLOCKS: [(&str, u32); 4] = [
    ("refinedstorage:1k_storage_block", 1_000),
    ("refinedstorage:4k_storage_block", 4_000),
    ("refinedstorage:16k_storage_block", 16_000),
    ("refinedstorage:64k_storage_block", 64_000),
];

/// The most blocks one network walk visits, so a huge cable net costs a bounded time.
pub const MAX_NODES: usize = 4_096;

/// One storage's contents: entries of something and how many, never more than `capacity` in all.
/// Refined Storage limits items, not types.
#[derive(Clone, Debug, PartialEq)]
pub struct Disk<K> {
    pub capacity: u32,
    pub entries: Vec<(K, u32)>,
}

impl<K: Clone> Disk<K> {
    pub fn stored(&self) -> u32 {
        self.entries.iter().map(|(_, n)| n).sum()
    }
    fn room(&self) -> u32 {
        self.capacity.saturating_sub(self.stored())
    }
    fn holds(&self, item: &K, same: &impl Fn(&K, &K) -> bool) -> bool {
        self.entries.iter().any(|(k, _)| same(k, item))
    }
    /// Puts up to `amount` of `item` in: how many went.
    pub fn insert(&mut self, item: &K, amount: u32, same: &impl Fn(&K, &K) -> bool) -> u32 {
        let went = amount.min(self.room());
        if went == 0 {
            return 0;
        }
        match self.entries.iter_mut().find(|(k, _)| same(k, item)) {
            Some((_, n)) => *n += went,
            None => self.entries.push((item.clone(), went)),
        }
        went
    }
    /// Takes up to `amount` of `item` out: how many came. An emptied entry goes.
    pub fn extract(&mut self, item: &K, amount: u32, same: &impl Fn(&K, &K) -> bool) -> u32 {
        let Some(at) = self.entries.iter().position(|(k, _)| same(k, item)) else {
            return 0;
        };
        let came = amount.min(self.entries[at].1);
        self.entries[at].1 -= came;
        if self.entries[at].1 == 0 {
            self.entries.remove(at);
        }
        came
    }
}

/// Inserts into a network's disks as Refined Storage fills them: first the disks that already
/// hold the item, then any with room, each in network order. Returns how many went in and marks
/// the disks it changed.
pub fn network_insert<K: Clone>(
    disks: &mut [Disk<K>],
    changed: &mut [bool],
    item: &K,
    amount: u32,
    same: &impl Fn(&K, &K) -> bool,
) -> u32 {
    let mut went = 0;
    for pass in [true, false] {
        for (i, disk) in disks.iter_mut().enumerate() {
            if went == amount {
                return went;
            }
            if pass && !disk.holds(item, same) {
                continue;
            }
            let n = disk.insert(item, amount - went, same);
            changed[i] |= n > 0;
            went += n;
        }
    }
    went
}

/// Takes up to `amount` of `item` from the network's disks, in network order.
pub fn network_extract<K: Clone>(
    disks: &mut [Disk<K>],
    changed: &mut [bool],
    item: &K,
    amount: u32,
    same: &impl Fn(&K, &K) -> bool,
) -> u32 {
    let mut came = 0;
    for (i, disk) in disks.iter_mut().enumerate() {
        if came == amount {
            break;
        }
        let n = disk.extract(item, amount - came, same);
        changed[i] |= n > 0;
        came += n;
    }
    came
}

/// What a block is to the network.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Node {
    /// Carries the network on (a Cable).
    Cable,
    /// Stores items and carries the network on, holding `capacity` items.
    Storage(u32),
    /// Another Controller: two on one network conflict, and the network stops working.
    Controller,
}

const NEIGHBOURS: [[i32; 3]; 6] = [
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
    [-1, 0, 0],
    [1, 0, 0],
];

/// The Storage Blocks a Controller at `start` reaches, in the order a breadth-first walk finds
/// them (down, up, north, south, west, east first), with their capacities. Every network block
/// carries the connection on; anything else ends it. None when the walk reaches a second
/// Controller.
pub fn discover(
    start: [i32; 3],
    node_at: impl Fn([i32; 3]) -> Option<Node>,
    limit: usize,
) -> Option<Vec<([i32; 3], u32)>> {
    let mut seen = HashSet::from([start]);
    let mut queue = VecDeque::from([start]);
    let mut storages = Vec::new();
    while let Some(pos) = queue.pop_front() {
        for d in NEIGHBOURS {
            let next = [pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]];
            if seen.len() >= limit || !seen.insert(next) {
                continue;
            }
            match node_at(next) {
                Some(Node::Cable) => queue.push_back(next),
                Some(Node::Storage(capacity)) => {
                    storages.push((next, capacity));
                    queue.push_back(next);
                }
                Some(Node::Controller) => return None,
                None => {}
            }
        }
    }
    Some(storages)
}

/// What one stored entry is: an item of this game, or one it no longer knows (its mod was
/// removed), kept as saved so it is not lost and still counted.
#[derive(Clone)]
enum Stored {
    Item(ItemResource),
    Unknown(Compound),
}

fn same(a: &Stored, b: &Stored) -> bool {
    match (a, b) {
        (Stored::Item(a), Stored::Item(b)) => a.matches(b.as_stack()),
        _ => false,
    }
}

struct Keys {
    controller: BlockKey,
    cable: BlockKey,
    storages: Vec<(BlockKey, u32)>,
}

static KEYS: OnceLock<Keys> = OnceLock::new();

/// A Storage Block's contents, read from its saved compound.
fn decode(host: &mut dyn ServerHost, data: &Compound, capacity: u32) -> Disk<Stored> {
    let mut entries = Vec::new();
    if let Some(Tag::List(list)) = data.get(&"stored".into()) {
        for tag in list {
            let Tag::Compound(entry) = tag else { continue };
            let (Some(Tag::Compound(item)), Some(Tag::Int(amount))) =
                (entry.get(&"item".into()), entry.get(&"amount".into()))
            else {
                continue;
            };
            if *amount <= 0 {
                continue;
            }
            let stored = match ItemStack::from_nbt(&*host, item) {
                Some(stack) => Stored::Item(ItemResource::of(&stack)),
                None => Stored::Unknown(item.clone()),
            };
            entries.push((stored, *amount as u32));
        }
    }
    Disk { capacity, entries }
}

fn encode(host: &mut dyn ServerHost, disk: &Disk<Stored>) -> Compound {
    let list = disk
        .entries
        .iter()
        .filter_map(|(stored, amount)| {
            let item = match stored {
                Stored::Item(item) => item.as_stack().to_nbt(&*host)?,
                Stored::Unknown(raw) => raw.clone(),
            };
            let entry: Compound = [
                ("item".into(), Tag::Compound(item)),
                ("amount".into(), Tag::Int(*amount as i32)),
            ]
            .into();
            Some(Tag::Compound(entry))
        })
        .collect();
    [("stored".into(), Tag::List(list))].into()
}

/// The network a Controller reaches: its Storage Blocks' positions and contents.
struct Loaded {
    at: Vec<[i32; 3]>,
    disks: Vec<Disk<Stored>>,
}

/// The network's items as the Controller answers them: every stored type is one index, and one
/// more index past them takes any new item.
struct NetworkItems {
    level: LevelRef,
    controller: [i32; 3],
}

impl NetworkItems {
    fn load(&self, host: &mut dyn ServerHost) -> Loaded {
        let Some(keys) = KEYS.get() else {
            return Loaded {
                at: Vec::new(),
                disks: Vec::new(),
            };
        };
        let level = self.level;
        let found = {
            let host: &dyn ServerHost = host;
            discover(
                self.controller,
                |pos| {
                    let block = level.block(host, pos)?;
                    if block == keys.cable {
                        return Some(Node::Cable);
                    }
                    if block == keys.controller {
                        return Some(Node::Controller);
                    }
                    keys.storages
                        .iter()
                        .find_map(|&(k, c)| (k == block).then_some(Node::Storage(c)))
                },
                MAX_NODES,
            )
            .unwrap_or_default()
        };
        let mut loaded = Loaded {
            at: Vec::new(),
            disks: Vec::new(),
        };
        for (pos, capacity) in found {
            let Some(data) = host
                .block_entity(level, pos)
                .and_then(|entity| entity.data(&*host))
            else {
                continue;
            };
            let disk = decode(host, &data, capacity);
            loaded.at.push(pos);
            loaded.disks.push(disk);
        }
        loaded
    }

    fn save(&self, host: &mut dyn ServerHost, loaded: &Loaded, changed: &[bool]) {
        for (i, disk) in loaded.disks.iter().enumerate() {
            if !changed[i] {
                continue;
            }
            let data = encode(host, disk);
            if let Some(entity) = host.block_entity(self.level, loaded.at[i]) {
                let _ = entity.merge_data(host, data);
            }
        }
    }

    /// The `index`th stored entry across the network.
    fn entry(loaded: &Loaded, index: usize) -> Option<&(Stored, u32)> {
        loaded
            .disks
            .iter()
            .flat_map(|d| d.entries.iter())
            .nth(index)
    }

    fn insert_item(&self, host: &mut dyn ServerHost, item: &ItemResource, amount: u32) -> u32 {
        let mut loaded = self.load(host);
        let mut changed = vec![false; loaded.disks.len()];
        let item = Stored::Item(item.clone());
        let went = network_insert(&mut loaded.disks, &mut changed, &item, amount, &same);
        self.save(host, &loaded, &changed);
        went
    }

    fn extract_item(&self, host: &mut dyn ServerHost, item: &ItemResource, amount: u32) -> u32 {
        let mut loaded = self.load(host);
        let mut changed = vec![false; loaded.disks.len()];
        let item = Stored::Item(item.clone());
        let came = network_extract(&mut loaded.disks, &mut changed, &item, amount, &same);
        self.save(host, &loaded, &changed);
        came
    }
}

impl ResourceHandler<ItemResource> for NetworkItems {
    fn size(&self, host: &mut dyn ServerHost) -> usize {
        let loaded = self.load(host);
        loaded.disks.iter().map(|d| d.entries.len()).sum::<usize>() + 1
    }
    fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<ItemResource> {
        match Self::entry(&self.load(host), index)? {
            (Stored::Item(item), _) => Some(item.clone()),
            (Stored::Unknown(_), _) => None,
        }
    }
    fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
        Self::entry(&self.load(host), index).map_or(0, |(_, n)| *n)
    }
    fn capacity(&self, host: &mut dyn ServerHost, _: usize, _: &ItemResource) -> u32 {
        self.load(host).disks.iter().map(|d| d.capacity).sum()
    }
    fn insert(
        &self,
        host: &mut dyn ServerHost,
        _: &Transaction,
        index: usize,
        resource: &ItemResource,
        amount: u32,
    ) -> u32 {
        // The network decides where an item goes; an index only says which item it holds.
        match Self::entry(&self.load(host), index) {
            Some((stored, _)) if !same(stored, &Stored::Item(resource.clone())) => 0,
            _ => self.insert_item(host, resource, amount),
        }
    }
    fn extract(
        &self,
        host: &mut dyn ServerHost,
        _: &Transaction,
        index: usize,
        resource: &ItemResource,
        amount: u32,
    ) -> u32 {
        match Self::entry(&self.load(host), index) {
            Some((stored, _)) if same(stored, &Stored::Item(resource.clone())) => {
                self.extract_item(host, resource, amount)
            }
            _ => 0,
        }
    }
    fn insert_any(
        &self,
        host: &mut dyn ServerHost,
        _: &Transaction,
        resource: &ItemResource,
        amount: u32,
    ) -> u32 {
        self.insert_item(host, resource, amount)
    }
    fn extract_any(
        &self,
        host: &mut dyn ServerHost,
        _: &Transaction,
        resource: &ItemResource,
        amount: u32,
    ) -> u32 {
        self.extract_item(host, resource, amount)
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let controller = r.block_key("refinedstorage:controller");
        let keys = Keys {
            controller,
            cable: r.block_key("refinedstorage:cable"),
            storages: STORAGE_BLOCKS
                .iter()
                .map(|&(id, capacity)| (r.block_key(id), capacity))
                .collect(),
        };
        let _ = KEYS.set(keys);
        let paths: Vec<&str> = STORAGE_BLOCKS
            .iter()
            .map(|(id, _)| id.trim_start_matches("refinedstorage:"))
            .collect();
        r.block_entity_type("storage_block").blocks(&paths).build();
        r.block_capability(&ITEMS, &controller, |_, side, _, _| {
            Some(Box::new(NetworkItems {
                level: side.level,
                controller: side.pos,
            }))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn eq(a: &&str, b: &&str) -> bool {
        a == b
    }

    #[test]
    fn a_disk_counts_items_not_stacks_or_types() {
        let mut disk = Disk {
            capacity: 1_000,
            entries: Vec::new(),
        };
        assert_eq!(disk.insert(&"cobblestone", 700, &eq), 700);
        assert_eq!(disk.insert(&"ender_pearl", 200, &eq), 200);
        // Only 100 items of room are left, whatever the item stacks to.
        assert_eq!(disk.insert(&"diamond", 250, &eq), 100);
        assert_eq!(disk.stored(), 1_000);
        assert_eq!(disk.extract(&"ender_pearl", 500, &eq), 200);
        assert_eq!(disk.entries.len(), 2);
        assert_eq!(disk.extract(&"dirt", 1, &eq), 0);
    }

    #[test]
    fn the_network_fills_disks_holding_the_item_first_then_any_with_room() {
        let mut disks = vec![
            Disk {
                capacity: 100,
                entries: vec![("dirt", 10)],
            },
            Disk {
                capacity: 100,
                entries: vec![("stone", 90)],
            },
        ];
        let mut changed = [false; 2];
        assert_eq!(
            network_insert(&mut disks, &mut changed, &"stone", 50, &eq),
            50
        );
        // 10 into the disk already holding stone, the other 40 where there is room.
        assert_eq!(disks[1].entries, vec![("stone", 100)]);
        assert_eq!(disks[0].entries, vec![("dirt", 10), ("stone", 40)]);
        assert_eq!(changed, [true, true]);
        let mut changed = [false; 2];
        assert_eq!(
            network_extract(&mut disks, &mut changed, &"stone", 45, &eq),
            45
        );
        assert_eq!(disks[0].entries, vec![("dirt", 10)]);
        assert_eq!(disks[1].entries, vec![("stone", 95)]);
        assert_eq!(
            network_insert(&mut disks, &mut [false; 2], &"gold", 500, &eq),
            95
        );
    }

    #[test]
    fn the_walk_follows_cables_and_storages_and_stops_at_a_gap() {
        // controller at the origin; cable east to a 1k at x=2, which passes on to a 4k at x=3;
        // a 16k at x=5 sits behind a gap.
        let world: HashMap<[i32; 3], Node> = HashMap::from([
            ([1, 0, 0], Node::Cable),
            ([2, 0, 0], Node::Storage(1_000)),
            ([3, 0, 0], Node::Storage(4_000)),
            ([5, 0, 0], Node::Storage(16_000)),
            ([0, 1, 0], Node::Cable),
            ([0, 2, 0], Node::Storage(64_000)),
        ]);
        let found = discover([0, 0, 0], |p| world.get(&p).copied(), MAX_NODES);
        assert_eq!(
            found,
            Some(vec![
                ([0, 2, 0], 64_000),
                ([2, 0, 0], 1_000),
                ([3, 0, 0], 4_000)
            ])
        );
        // A walk capped at the controller and its six neighbours never reaches x=2.
        let found = discover([0, 0, 0], |p| world.get(&p).copied(), 7);
        assert_eq!(found, Some(Vec::new()));
        // A second controller on the cable at x=4 would join the 16k: the network conflicts.
        let mut world = world;
        world.insert([4, 0, 0], Node::Controller);
        assert_eq!(
            discover([0, 0, 0], |p| world.get(&p).copied(), MAX_NODES),
            None
        );
    }
}
