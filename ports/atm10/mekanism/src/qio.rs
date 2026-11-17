//! The QIO drive array, as a stand-in: one block that stores up to 128 kinds of item, each up
//! to two billion, for pipes, transporters and hoppers (the `ITEMS` capability). The original
//! is a network of an array of drives, a dashboard, importers and exporters on a frequency;
//! none of that exists here yet (no dashboard screen, no drives to insert, no wireless access).
//! See README.
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockPart, BlockSide, Compound, Host, ITEMS, ItemResource,
    ItemStack, LevelRef, LevelSide, Registrar, ResourceHandler, ServerHost, Tag, Transaction,
};

pub const ARRAY: &str = "mekanism:qio_drive_array";
pub const TYPES: usize = 128;
/// Items of one kind the array holds at most.
pub const PER_TYPE: u32 = i32::MAX as u32;

/// What the array holds: a kind in at most one slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slots<K> {
    pub slots: Vec<Option<(K, u32)>>,
}

impl<K: PartialEq + Clone> Slots<K> {
    pub fn new(n: usize) -> Self {
        Slots { slots: vec![None; n] }
    }

    /// Puts up to `amount` of `key` in slot `i`: how many went in. A kind already in another
    /// slot is refused there.
    pub fn insert(&mut self, i: usize, key: &K, amount: u32) -> u32 {
        if self.slots.iter().enumerate().any(|(j, s)| j != i && s.as_ref().is_some_and(|(k, _)| k == key)) {
            return 0;
        }
        let Some(slot) = self.slots.get_mut(i) else { return 0 };
        match slot {
            None => {
                let put = amount.min(PER_TYPE);
                if put > 0 {
                    *slot = Some((key.clone(), put));
                }
                put
            }
            Some((k, n)) if k == key => {
                let put = amount.min(PER_TYPE - *n);
                *n += put;
                put
            }
            Some(_) => 0,
        }
    }

    /// Takes up to `amount` of `key` out of slot `i`; an emptied slot is free again.
    pub fn extract(&mut self, i: usize, key: &K, amount: u32) -> u32 {
        let Some(slot) = self.slots.get_mut(i) else { return 0 };
        match slot {
            Some((k, n)) if k == key => {
                let took = amount.min(*n);
                *n -= took;
                if *n == 0 {
                    *slot = None;
                }
                took
            }
            _ => 0,
        }
    }
}

fn load(host: &dyn Host, nbt: &Compound) -> Slots<ItemResource> {
    let mut slots = Slots::new(TYPES);
    if let Some(Tag::List(list)) = nbt.get(&"slots".into()) {
        for (i, entry) in list.iter().take(TYPES).enumerate() {
            let Tag::Compound(c) = entry else { continue };
            let count = match c.get(&"count".into()) {
                Some(Tag::Int(v)) => (*v).max(0) as u32,
                _ => 0,
            };
            let stack = match c.get(&"item".into()) {
                Some(Tag::Compound(item)) => ItemStack::from_nbt(host, item),
                _ => None,
            };
            if let (Some(stack), true) = (stack, count > 0) {
                slots.slots[i] = Some((ItemResource::of(&stack), count));
            }
        }
    }
    slots
}

fn save(host: &dyn Host, slots: &Slots<ItemResource>) -> Compound {
    let list = slots
        .slots
        .iter()
        .map(|s| {
            let mut c = Compound::new();
            if let Some((res, count)) = s {
                if let Some(item) = res.to_stack(1).and_then(|s| s.to_nbt(host)) {
                    c.insert("item".into(), Tag::Compound(item));
                    c.insert("count".into(), Tag::Int(*count as i32));
                }
            }
            Tag::Compound(c)
        })
        .collect();
    let mut nbt = Compound::new();
    nbt.insert("slots".into(), Tag::List(list));
    nbt
}

struct Array {
    level: LevelRef,
    pos: [i32; 3],
}

impl Array {
    fn read(&self, host: &mut dyn ServerHost) -> Option<Slots<ItemResource>> {
        let entity = host.block_entity(self.level, self.pos)?;
        Some(load(&*host, &entity.data(&*host)?))
    }

    fn write(&self, host: &mut dyn ServerHost, slots: &Slots<ItemResource>) {
        if let Some(entity) = host.block_entity(self.level, self.pos) {
            let patch = save(&*host, slots);
            let _ = entity.merge_data(host, patch);
        }
    }
}

impl ResourceHandler<ItemResource> for Array {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        TYPES
    }
    fn resource(&self, host: &mut dyn ServerHost, i: usize) -> Option<ItemResource> {
        self.read(host)?.slots.get(i)?.as_ref().map(|(r, _)| r.clone())
    }
    fn amount(&self, host: &mut dyn ServerHost, i: usize) -> u32 {
        self.read(host).and_then(|s| s.slots.get(i).cloned().flatten()).map_or(0, |(_, n)| n)
    }
    fn capacity(&self, _: &mut dyn ServerHost, _: usize, _: &ItemResource) -> u32 {
        PER_TYPE
    }
    fn insert(&self, host: &mut dyn ServerHost, _tx: &Transaction, i: usize, r: &ItemResource, amount: u32) -> u32 {
        let Some(mut slots) = self.read(host) else { return 0 };
        let put = slots.insert(i, r, amount);
        if put > 0 {
            self.write(host, &slots);
        }
        put
    }
    fn extract(&self, host: &mut dyn ServerHost, _tx: &Transaction, i: usize, r: &ItemResource, amount: u32) -> u32 {
        let Some(mut slots) = self.read(host) else { return 0 };
        let took = slots.extract(i, r, amount);
        if took > 0 {
            self.write(host, &slots);
        }
        took
    }
}

struct Part;

#[mod_api::horizon_part]
impl BlockPart for Part {}

#[mod_api::horizon_part]
impl BlockEntityPart for Part {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        (event.block == ARRAY).then(Compound::new)
    }
}

pub fn register(r: &mut dyn Registrar) {
    let built = r.block_entity_type("qio_drive_array").blocks(&[ARRAY]).build();
    let _ = r.block_entity_part(&built, Part);
    r.block_entity_capability(&ITEMS, &built, |_, at: BlockSide| {
        Some(Box::new(Array { level: at.level, pos: at.pos }) as Box<dyn ResourceHandler<ItemResource>>)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kind_lives_in_one_slot_and_an_emptied_slot_is_free() {
        let mut s = Slots::new(4);
        assert_eq!(s.insert(0, &"cobble", 5_000), 5_000);
        // another slot refuses the same kind
        assert_eq!(s.insert(1, &"cobble", 10), 0);
        assert_eq!(s.insert(1, &"dirt", 7), 7);
        assert_eq!(s.extract(0, &"cobble", 6_000), 5_000);
        assert_eq!(s.slots[0], None);
        assert_eq!(s.insert(0, &"cobble", 3), 3);
    }

    #[test]
    fn a_kind_stops_at_two_billion() {
        let mut s = Slots::new(1);
        assert_eq!(s.insert(0, &"cobble", u32::MAX), PER_TYPE);
        assert_eq!(s.insert(0, &"cobble", 1), 0);
    }
}
