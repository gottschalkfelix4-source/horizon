//! Functional Storage for Horizon: wooden drawers that hold many items of one kind.
//!
//! Written from Functional Storage's observed behaviour and public description, never from its
//! code (docs/modpacks.md, "Licences"). README.md lists what is ported and what a check against
//! the NeoForge original still has to confirm.
//!
//! `functionalstorage:<wood>_<1|2|4>` are blocks with a block entity of one type. Its slots live
//! in the saved `slots` list (item, count); a capability provider answers `ITEMS` for hoppers,
//! pipes and every other mod, with a slot holding 32, 16 or 8 stacks. Using a drawer with an item
//! puts the held stack in; using it with an empty hand takes a stack (one when sneaking).
use mod_api::{
    BlockEntityEvent, BlockEntityPart, BlockPart, BlockSide, Compound, Hooks, HostTransaction,
    InteractionResult, ItemPlace, ItemResource, ItemStack, LevelRef, LevelSide, Registrar,
    RegistrationContext, ResourceHandler, ServerHost, Tag, Transaction, UseItemOnEvent,
    UseWithoutItemEvent, ITEMS,
};

pub mod rules;
use rules::*;

fn int(nbt: &Compound, key: &str) -> u32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => (*v).max(0) as u32,
        _ => 0,
    }
}

/// The drawer's slots as saved: a list with one compound per slot, empty when the slot is.
fn load(host: &dyn mod_api::Host, nbt: &Compound, n: usize) -> Slots<ItemResource> {
    let mut slots = Slots::new(n);
    if let Some(Tag::List(list)) = nbt.get(&"slots".into()) {
        for (i, entry) in list.iter().take(n).enumerate() {
            let Tag::Compound(c) = entry else { continue };
            let count = int(c, "count");
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

fn save(host: &dyn mod_api::Host, slots: &Slots<ItemResource>) -> Compound {
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

/// What `ITEMS` gives of a drawer.
struct Drawer {
    level: LevelRef,
    pos: [i32; 3],
}

impl Drawer {
    fn read(&self, host: &mut dyn ServerHost) -> Option<Slots<ItemResource>> {
        let n = slots_of(&self.level.block(&*host, self.pos)?.id(&*host))?;
        let entity = host.block_entity(self.level, self.pos)?;
        Some(load(&*host, &entity.data(&*host)?, n))
    }

    fn write(&self, host: &mut dyn ServerHost, slots: &Slots<ItemResource>) {
        if let Some(entity) = host.block_entity(self.level, self.pos) {
            let patch = save(&*host, slots);
            let _ = entity.merge_data(host, patch);
        }
    }

    fn capacity_of(&self, host: &mut dyn ServerHost, n: usize, res: &ItemResource) -> u32 {
        host.max_stack_size(res.item()).unwrap_or(64) * stacks_per_slot(n)
    }
}

impl ResourceHandler<ItemResource> for Drawer {
    fn size(&self, host: &mut dyn ServerHost) -> usize {
        self.read(host).map_or(0, |s| s.slots.len())
    }

    fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<ItemResource> {
        self.read(host)?.slots.get(index)?.as_ref().map(|(r, _)| r.clone())
    }

    fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
        self.read(host)
            .and_then(|s| s.slots.get(index).cloned().flatten())
            .map_or(0, |(_, n)| n)
    }

    fn capacity(&self, host: &mut dyn ServerHost, _index: usize, resource: &ItemResource) -> u32 {
        let n = self.read(host).map_or(1, |s| s.slots.len());
        self.capacity_of(host, n, resource)
    }

    fn insert(&self, host: &mut dyn ServerHost, _tx: &Transaction, index: usize, resource: &ItemResource, amount: u32) -> u32 {
        let Some(mut slots) = self.read(host) else { return 0 };
        let cap = self.capacity_of(host, slots.slots.len(), resource);
        let put = slots.insert(index, resource, amount, cap);
        if put > 0 {
            self.write(host, &slots);
        }
        put
    }

    fn extract(&self, host: &mut dyn ServerHost, _tx: &Transaction, index: usize, resource: &ItemResource, amount: u32) -> u32 {
        let Some(mut slots) = self.read(host) else { return 0 };
        let took = slots.extract(index, resource, amount);
        if took > 0 {
            self.write(host, &slots);
        }
        took
    }
}

struct Drawers;

#[mod_api::horizon_part]
impl BlockPart for Drawers {
    /// A held stack goes into the drawer; the rest stays in the hand.
    fn use_item_on(&self, host: &mut dyn ServerHost, event: &mut UseItemOnEvent<'_>) -> Option<InteractionResult> {
        let click = event.click;
        let level = host.level(host.dimension())?;
        slots_of(&level.block(&*host, click.pos)?.id(&*host))?;
        let held = click.stack?.clone();
        let cap = level.capability(host, &ITEMS, click.pos, None)?;
        let hand = ItemPlace::PlayerHand { player: click.player, hand: click.hand };
        let res = ItemResource::of(&held);
        let put = host
            .transaction(|host, tx| {
                let taken = hand.extract(host, tx, &res, held.count());
                let put = cap.insert_any(host, tx, &res, taken);
                if taken > put {
                    hand.insert(host, tx, &res, taken - put);
                }
                Ok::<u32, ()>(put)
            })
            .unwrap_or(0);
        Some(if put > 0 { InteractionResult::SuccessServer } else { InteractionResult::Fail })
    }

    /// An empty hand takes a stack from the last filled slot, or one item when sneaking.
    fn use_without_item(&self, host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
        let click = event.click;
        let level = host.level(host.dimension())?;
        slots_of(&level.block(&*host, click.pos)?.id(&*host))?;
        let player = host.player(click.player)?;
        let one = player.entity().is_shift_key_down(&*host).unwrap_or(false);
        let cap = level.capability(host, &ITEMS, click.pos, None)?;
        let index = (0..cap.size(host)).rev().find(|i| cap.resource(host, *i).is_some())?;
        let res = cap.resource(host, index)?;
        let want = if one { 1 } else { host.max_stack_size(res.item()).unwrap_or(64) };
        let took = host
            .transaction(|host, tx| Ok::<u32, ()>(cap.extract(host, tx, index, &res, want)))
            .unwrap_or(0);
        let stack = res.to_stack(took)?;
        let _ = player.give_stack(host, stack);
        Some(InteractionResult::SuccessServer)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for Drawers {
    fn block_entity(&self, _: LevelSide<'_>, event: &mut BlockEntityEvent<'_>) -> Option<Compound> {
        let n = slots_of(event.block)?;
        let mut nbt = Compound::new();
        nbt.insert("slots".into(), Tag::List(vec![Tag::Compound(Compound::new()); n]));
        Some(nbt)
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let ids: Vec<String> = WOODS
            .iter()
            .flat_map(|w| SIZES.iter().map(move |n| format!("functionalstorage:{w}_{n}")))
            .collect();
        let names: Vec<&str> = ids.iter().map(String::as_str).collect();
        for id in &ids {
            let key = r.block_key(id);
            r.block_part(key, Drawers);
        }
        let kind = r.block_entity_type("drawer").blocks(&names).build();
        let _ = r.block_entity_part(&kind, Drawers);
        r.block_entity_capability(&ITEMS, &kind, |_, at: BlockSide| {
            Some(Box::new(Drawer { level: at.level, pos: at.pos }) as Box<dyn ResourceHandler<ItemResource>>)
        });
    }
}
