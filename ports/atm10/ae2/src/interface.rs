//! The ME interface: the network seen as an inventory by the blocks around it. A hopper, a machine or a
//! bus of another mod that reaches the interface's item capability finds 54 virtual slots listing the
//! network's items by count; taking from one takes from the network, and anything put in goes into
//! the network's cells. It needs power and a channel; it refuses stacks with components.
use mod_api::{ItemResource, LevelRef, ResourceHandler, ServerHost, Transaction};

use crate::{network_extract, network_insert};

pub const SLOTS: usize = 54;

pub struct Interface {
    pub level: LevelRef,
    pub pos: [i32; 3],
}

impl Interface {
    /// The network's items, most first (empty without power or a channel), and its drives.
    fn view(&self, host: &mut dyn ServerHost) -> (Vec<(String, u64)>, Vec<[i32; 3]>) {
        let n = crate::terminal::net(host, self.level, self.pos);
        if !n.powered || !n.channel {
            return (Vec::new(), Vec::new());
        }
        let list = crate::terminal::listing_of(&*host, self.level, &n);
        (list, n.drives)
    }
}

fn plain(res: &ItemResource) -> bool {
    res.to_stack(1).is_some_and(|s| s.components().is_none_or(|c| c.as_nbt().is_empty()))
}

impl ResourceHandler<ItemResource> for Interface {
    fn size(&self, _host: &mut dyn ServerHost) -> usize {
        SLOTS
    }

    fn resource(&self, host: &mut dyn ServerHost, index: usize) -> Option<ItemResource> {
        let (list, _) = self.view(host);
        let (id, n) = list.get(index)?;
        if *n == 0 {
            return None;
        }
        let key = host.item_key(&mod_api::Identifier::parse(id).ok()?)?;
        Some(ItemResource::of(&mod_api::ItemStack::new(key, 1)?))
    }

    fn amount(&self, host: &mut dyn ServerHost, index: usize) -> u32 {
        let (list, _) = self.view(host);
        list.get(index).map_or(0, |(_, n)| (*n).min(u32::MAX as u64) as u32)
    }

    fn capacity(&self, host: &mut dyn ServerHost, _index: usize, resource: &ItemResource) -> u32 {
        host.max_stack_size(resource.item()).unwrap_or(64)
    }

    fn insert(&self, host: &mut dyn ServerHost, _tx: &Transaction, _index: usize, resource: &ItemResource, amount: u32) -> u32 {
        if !plain(resource) {
            return 0;
        }
        let Some(stack) = resource.to_stack(1) else { return 0 };
        let id = stack.item().id(&*host);
        let (_, drives) = self.view(host);
        network_insert(host, self.level, &drives, &id, amount as u64) as u32
    }

    fn extract(&self, host: &mut dyn ServerHost, _tx: &Transaction, _index: usize, resource: &ItemResource, amount: u32) -> u32 {
        let Some(stack) = resource.to_stack(1) else { return 0 };
        let id = stack.item().id(&*host);
        let (_, drives) = self.view(host);
        network_extract(host, self.level, &drives, &id, amount as u64) as u32
    }
}
