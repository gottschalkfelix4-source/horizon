//! The world's shared storages: one inventory or tank per frequency, which every block of that
//! frequency shows, in every dimension.
//!
//! The game keeps a block's slots and tanks in its block entity, which hoppers, pipes and the
//! vanilla chest menu read and write. Each block entity keeps a copy of its frequency's storage
//! and the revision it last saw; every tick it compares the two ([`reconcile`]): an edit of its
//! copy is pushed to the shared storage (a new revision), and a newer revision is pulled into the
//! copy. Two blocks of one frequency edited in the same tick: the one ticking later loses its
//! edit (README, "Needs from Horizon").
//!
//! The shared storages are saved in the world, in `horizon-mod-data/enderstorage/<file>`, at the
//! end of a server tick that changed them, and read back when a world opens.
use std::collections::BTreeMap;
use std::path::PathBuf;

use mod_api::{Compound, ConfigPath, Host, ItemStack, ServerHost, Tag};

/// One shared storage and its revision, raised by every push.
#[derive(Clone, Debug, PartialEq)]
pub struct Shared<T> {
    pub rev: i64,
    pub value: T,
}

/// What [`reconcile`] did to a block entity's copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sync {
    /// The copy is the shared storage's current revision, unchanged.
    Same,
    /// The copy was edited: it is now the shared storage, at a new revision.
    Pushed,
    /// The shared storage was newer: the copy took it (`changed` when its contents differed).
    Pulled { changed: bool },
}

/// Bring a block entity's copy `local` (last synced at revision `local_rev`, `None` when it
/// never was or its frequency changed) and the shared storage together.
///
/// A copy ahead of the shared storage (the save was lost, or the block came from another world)
/// is taken as the storage, so a block entity is never emptied by a missing file.
pub fn reconcile<T: PartialEq + Clone>(
    local: &mut T,
    local_rev: &mut Option<i64>,
    shared: &mut Shared<T>,
) -> Sync {
    match *local_rev {
        Some(rev) if rev == shared.rev => {
            if *local == shared.value {
                Sync::Same
            } else {
                shared.value = local.clone();
                shared.rev += 1;
                *local_rev = Some(shared.rev);
                Sync::Pushed
            }
        }
        Some(rev) if rev > shared.rev => {
            shared.value = local.clone();
            shared.rev = rev;
            Sync::Pushed
        }
        _ => {
            let changed = *local != shared.value;
            if changed {
                *local = shared.value.clone();
            }
            *local_rev = Some(shared.rev);
            Sync::Pulled { changed }
        }
    }
}

/// How a storage's contents go to and from its saved compound.
pub trait Codec: Sized {
    fn encode(&self, host: &dyn Host) -> Tag;
    fn decode(host: &dyn Host, tag: &Tag) -> Option<Self>;
}

/// Item slots, saved as a list of `{Slot: byte, ...stack}` for the stacks there are.
pub type Items = Vec<Option<ItemStack>>;

impl Codec for Items {
    fn encode(&self, host: &dyn Host) -> Tag {
        let mut list = Vec::new();
        for (slot, stack) in self.iter().enumerate() {
            if let Some(nbt) = stack.as_ref().and_then(|s| s.to_nbt(host)) {
                let mut nbt = nbt;
                nbt.insert("Slot".into(), Tag::Int(slot as i32));
                list.push(Tag::Compound(nbt));
            }
        }
        Tag::compound([
            ("size", Tag::Int(self.len() as i32)),
            ("items", Tag::List(list)),
        ])
    }

    fn decode(host: &dyn Host, tag: &Tag) -> Option<Self> {
        let Some(Tag::Int(size)) = tag.get("size") else {
            return None;
        };
        let mut items = vec![None; (*size).clamp(0, 1024) as usize];
        if let Some(Tag::List(list)) = tag.get("items") {
            for entry in list {
                let Tag::Compound(nbt) = entry else { continue };
                let Some(Tag::Int(slot)) = entry.get("Slot") else { continue };
                if let Some(cell) = items.get_mut(*slot as usize) {
                    *cell = ItemStack::from_nbt(host, nbt);
                }
            }
        }
        Some(items)
    }
}

/// A tank: its fluid's id and amount in millibuckets, as the game saves a mod tank.
pub type Tank = (Option<String>, i32);

impl Codec for Tank {
    fn encode(&self, _: &dyn Host) -> Tag {
        tank_tag(self)
    }
    fn decode(_: &dyn Host, tag: &Tag) -> Option<Self> {
        Some(tank_of(tag))
    }
}

/// One tank as the game's `fluids` list holds it (`{Fluid, Amount}`).
pub fn tank_tag(tank: &Tank) -> Tag {
    Tag::compound([
        (
            "Fluid",
            Tag::String(tank.0.as_deref().unwrap_or("minecraft:empty").into()),
        ),
        ("Amount", Tag::Int(tank.1)),
    ])
}

/// [`tank_tag`]'s reader: an empty tank for anything else.
pub fn tank_of(tag: &Tag) -> Tank {
    let fluid = match tag.get("Fluid") {
        Some(Tag::String(f)) => Some(f.to_string_lossy()).filter(|f| f != "minecraft:empty"),
        _ => None,
    };
    let amount = match tag.get("Amount") {
        Some(Tag::Int(a)) => (*a).max(0),
        _ => 0,
    };
    if amount == 0 { (None, 0) } else { (fluid, amount) }
}

/// The first tank of a block entity's saved `fluids` list.
pub fn first_tank(nbt: &Compound) -> Tank {
    match nbt.get(&"fluids".into()) {
        Some(Tag::List(list)) => list.first().map(tank_of).unwrap_or((None, 0)),
        _ => (None, 0),
    }
}

/// Write `tank` as the block entity's only tank.
pub fn set_first_tank(nbt: &mut Compound, tank: &Tank) {
    nbt.insert("fluids".into(), Tag::List(vec![tank_tag(tank)]));
}

/// The shared storages of one kind, for the world open now.
pub struct Store<T> {
    file: &'static str,
    world: Option<PathBuf>,
    loaded: bool,
    pub entries: BTreeMap<String, Shared<T>>,
    dirty: bool,
}

impl<T: Codec + Clone> Store<T> {
    pub const fn new(file: &'static str) -> Self {
        Self {
            file,
            world: None,
            loaded: false,
            entries: BTreeMap::new(),
            dirty: false,
        }
    }

    /// Make the store the open world's: read its file when the world changed.
    pub fn open(&mut self, host: &dyn ServerHost) {
        let world = host.world_dir();
        if self.loaded && world == self.world {
            return;
        }
        self.world = world;
        self.loaded = true;
        self.dirty = false;
        self.entries.clear();
        let Ok(path) = ConfigPath::new(self.file) else {
            return;
        };
        let Ok(bytes) = host.world_data_files().read(&path) else {
            return;
        };
        let Ok((root, _)) = Tag::from_network_bytes(&bytes, Some(u64::MAX)) else {
            return;
        };
        let Some(Tag::List(list)) = root.get("storages") else {
            return;
        };
        for entry in list {
            let (Some(Tag::String(key)), Some(Tag::Long(rev)), Some(value)) =
                (entry.get("key"), entry.get("rev"), entry.get("value"))
            else {
                continue;
            };
            if let Some(value) = T::decode(host, value) {
                self.entries
                    .insert(key.to_string_lossy(), Shared { rev: *rev, value });
            }
        }
    }

    /// The storage of `key`, made with `empty` contents the first time it is asked for.
    pub fn get(&mut self, key: &str, empty: impl FnOnce() -> T) -> &mut Shared<T> {
        self.entries
            .entry(key.to_owned())
            .or_insert_with(|| Shared { rev: 0, value: empty() })
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Save the store when something changed since the last save.
    pub fn flush(&mut self, host: &dyn ServerHost) {
        if !self.dirty || !self.loaded || host.world_dir() != self.world {
            return;
        }
        let list = self
            .entries
            .iter()
            .map(|(key, shared)| {
                Tag::compound([
                    ("key", Tag::String(key.as_str().into())),
                    ("rev", Tag::Long(shared.rev)),
                    ("value", shared.value.encode(host)),
                ])
            })
            .collect();
        let root = Tag::compound([("storages", Tag::List(list))]);
        let (Ok(bytes), Ok(path)) = (root.to_network_bytes(), ConfigPath::new(self.file)) else {
            return;
        };
        if host.world_data_files().write(&path, &bytes).is_ok() {
            self.dirty = false;
        }
    }
}

/// A revision as a block entity saves it: `rev`, -1 (or missing) for none.
pub fn load_rev(nbt: &Compound) -> Option<i64> {
    match nbt.get(&"rev".into()) {
        Some(Tag::Long(rev)) if *rev >= 0 => Some(*rev),
        _ => None,
    }
}

pub fn save_rev(nbt: &mut Compound, rev: Option<i64>) {
    nbt.insert("rev".into(), Tag::Long(rev.unwrap_or(-1)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edited_copy_is_pushed_and_raises_the_revision() {
        let mut shared = Shared { rev: 3, value: vec![1, 2] };
        let (mut local, mut rev) = (vec![1, 5], Some(3));
        assert_eq!(reconcile(&mut local, &mut rev, &mut shared), Sync::Pushed);
        assert_eq!((shared.rev, shared.value.clone(), rev), (4, vec![1, 5], Some(4)));
        assert_eq!(reconcile(&mut local, &mut rev, &mut shared), Sync::Same);
    }

    #[test]
    fn a_stale_copy_takes_the_newer_storage_and_loses_its_own_edit() {
        let mut shared = Shared { rev: 7, value: vec![9] };
        let (mut local, mut rev) = (vec![4], Some(6));
        assert_eq!(
            reconcile(&mut local, &mut rev, &mut shared),
            Sync::Pulled { changed: true }
        );
        assert_eq!((local, rev, shared.rev), (vec![9], Some(7), 7));
    }

    #[test]
    fn a_new_block_shows_the_storage_and_a_copy_ahead_of_it_is_kept() {
        let mut shared = Shared { rev: 2, value: vec![8] };
        let (mut local, mut rev) = (vec![0], None);
        reconcile(&mut local, &mut rev, &mut shared);
        assert_eq!((local, rev), (vec![8], Some(2)));

        let mut lost = Shared { rev: 0, value: vec![0] };
        let (mut local, mut rev) = (vec![5], Some(12));
        assert_eq!(reconcile(&mut local, &mut rev, &mut lost), Sync::Pushed);
        assert_eq!((lost.rev, lost.value), (12, vec![5]));
    }

    #[test]
    fn a_tank_reads_back_from_the_games_saved_form() {
        let mut nbt = Compound::new();
        set_first_tank(&mut nbt, &(Some("minecraft:lava".into()), 3000));
        assert_eq!(first_tank(&nbt), (Some("minecraft:lava".into()), 3000));
        set_first_tank(&mut nbt, &(Some("minecraft:water".into()), 0));
        assert_eq!(first_tank(&nbt), (None, 0));
    }
}
