//! Silent's Gems for Horizon: 48 gems across three sets, each with ores, a storage block,
//! bricks, glass, a glowrose and a teleporter; a Teleporter Linker pairs two teleporters and a
//! Return Home Charm, bound to a teleporter, takes you back to it.
//!
//! Written from Silent's Gems' public description and observed behaviour, never from its code
//! (docs/modpacks.md, "Licences"). What the port does, and what a harness against the NeoForge
//! original still has to confirm, is in README.md.
//!
//! How: the blocks and items are declared in code (`blocks`, `items`), one declaration per gem
//! from the catalogue in `gems.rs`. A teleporter's link is saved on its chunk (attachment
//! `silentgems:links`); the linker and the charm keep the teleporter they are bound to in the
//! `silentgems:bound` component.

pub mod gems;
pub mod links;

use std::sync::OnceLock;

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, Block, BlockKey, BlockPart, BlocksEvent,
    Component, ComponentTypesEvent, Compound, Holder, Hooks, Host, HostSide, InteractionResult,
    Item, ItemKey, ItemPart, ItemsEvent, LivingEntityUseItemFinishEvent, PlayerRef, Registrar,
    RegistrationContext, ServerHost, Tag, Text, UseDurationEvent, UseOnEvent, UseWithoutItemEvent,
};

use gems::{GEMS, LINKER, charm_id, teleporter_id};
use links::{Link, Place};

/// The teleporter a linker or charm is bound to.
pub struct BoundComponent;

pub const BOUND: BoundComponent = BoundComponent;

impl Component for BoundComponent {
    type Value = Place;
    fn id(&self) -> &str {
        "silentgems:bound"
    }
    fn decode(&self, value: &Tag) -> Option<Place> {
        match value {
            Tag::Compound(c) => Place::from_nbt(c),
            _ => None,
        }
    }
    fn encode(&self, value: Place) -> Tag {
        Tag::Compound(value.to_nbt())
    }
}

/// The teleporter links recorded on one chunk.
static LINKS: AttachmentType<Vec<Link>> = AttachmentType {
    namespace: "silentgems",
    name: "links",
    default: Vec::new,
    codec: Some(AttachmentCodec {
        encode: encode_links,
        decode: decode_links,
    }),
    copy_on_death: false,
    sync: false,
};

// The codec's signature is fixed by `AttachmentCodec` (`fn(&T)` with `T = Vec<Link>`).
#[allow(clippy::ptr_arg)]
fn encode_links(links: &Vec<Link>) -> Result<Compound, AttachmentError> {
    Ok(links::encode(links))
}

fn decode_links(data: &Compound) -> Result<Vec<Link>, AttachmentError> {
    links::decode(data).ok_or(AttachmentError::Decode)
}

/// How long the charm is held before it warps, in ticks (assumed).
pub const CHARM_USE_TICKS: i32 = 60;

struct Keys {
    teleporters: Vec<BlockKey>,
    charms: Vec<ItemKey>,
}

static KEYS: OnceLock<Keys> = OnceLock::new();

fn is_teleporter(block: Option<BlockKey>) -> bool {
    block.is_some_and(|b| KEYS.get().is_some_and(|k| k.teleporters.contains(&b)))
}

fn is_charm(item: ItemKey) -> bool {
    KEYS.get().is_some_and(|k| k.charms.contains(&item))
}

fn chunk_of(pos: [i32; 3]) -> Holder {
    Holder::Chunk([pos[0] >> 4, pos[2] >> 4])
}

/// The player's dimension and whether a teleporter stands at `pos` there.
fn teleporter_place(host: &dyn Host, player: PlayerRef, pos: [i32; 3]) -> Option<Place> {
    let level = player.entity().level(host)?;
    is_teleporter(level.block(host, pos)).then(|| Place {
        dimension: level.dimension_id().to_string(),
        pos,
    })
}

fn tell(host: &mut dyn ServerHost, player: PlayerRef, key: &str) {
    let _ = player.action_bar(host, Text::translate(key, Vec::new()));
}

/// Records that the teleporter at `from` (in the host's current level) sends to `to`.
fn set_link(host: &mut dyn ServerHost, from: &Place, to: Place) -> bool {
    let holder = chunk_of(from.pos);
    let Ok(links) = LINKS.get(host.attachments(), holder) else {
        return false;
    };
    LINKS
        .set(
            host.attachments(),
            holder,
            links::relink(links, from.pos, to),
        )
        .is_ok()
}

/// Sends `player` to the teleporter at `to`, if one still stands there.
fn warp(host: &mut dyn ServerHost, player: PlayerRef, to: &Place) -> bool {
    let Some(level) = host.level(&to.dimension) else {
        return false;
    };
    if !is_teleporter(level.block(host, to.pos)) {
        return false;
    }
    player
        .entity()
        .teleport_to_level(host, &to.dimension, to.arrival(), None)
}

/// The Teleporter Linker: first click binds a teleporter, the second links the two both ways.
struct LinkerItem;

#[mod_api::horizon_part]
impl ItemPart for LinkerItem {
    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        let player = host.player(event.player)?;
        let here = teleporter_place(host, player, event.pos)?;
        let living = host.living(player.entity())?;
        let mut stack = event.stack.clone();
        match stack.get(&BOUND) {
            Some(first) if first != here => {
                // Both ends: this level's now, the first one's when it is this level too.
                let linked = set_link(host, &here, first.clone())
                    && (first.dimension != here.dimension || set_link(host, &first, here.clone()));
                stack.remove(&BOUND);
                tell(
                    host,
                    player,
                    if linked {
                        "silentgems.linker.linked"
                    } else {
                        "silentgems.linker.failed"
                    },
                );
            }
            _ => {
                stack.set(&BOUND, here).ok()?;
                tell(host, player, "silentgems.linker.bound");
            }
        }
        let _ = living.set_hand_item(host, event.hand, Some(stack));
        Some(InteractionResult::Success)
    }
}

/// A Return Home Charm: bound by using it on a teleporter, then held to warp there.
struct CharmItem;

#[mod_api::horizon_part]
impl ItemPart for CharmItem {
    fn use_on(&self, host: HostSide, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        let player = host.player(event.player)?;
        let here = teleporter_place(host, player, event.pos)?;
        let living = host.living(player.entity())?;
        let mut stack = event.stack.clone();
        stack.set(&BOUND, here).ok()?;
        let _ = living.set_hand_item(host, event.hand, Some(stack));
        tell(host, player, "silentgems.charm.bound");
        Some(InteractionResult::Success)
    }

    /// A bound charm is held for a while before it warps; an unbound one is not used.
    fn use_duration(&self, _: &dyn Host, event: &mut UseDurationEvent<'_>) {
        event.duration = if event.stack.get(&BOUND).is_some() {
            CHARM_USE_TICKS
        } else {
            0
        };
    }
}

/// A gem teleporter: used with an empty hand, it sends the player along its link.
struct TeleporterPart;

#[mod_api::horizon_part]
impl BlockPart for TeleporterPart {
    fn use_without_item(
        &self,
        host: &mut dyn ServerHost,
        event: &mut UseWithoutItemEvent<'_>,
    ) -> Option<InteractionResult> {
        let player = host.player(event.click.player)?;
        let links = LINKS
            .get(host.attachments(), chunk_of(event.click.pos))
            .ok()?;
        let Some(link) = links.into_iter().find(|l| l.from == event.click.pos) else {
            tell(host, player, "silentgems.teleporter.unlinked");
            return Some(InteractionResult::Success);
        };
        if !warp(host, player, &link.to) {
            tell(host, player, "silentgems.teleporter.missing");
        }
        Some(InteractionResult::Success)
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let teleporters: Vec<BlockKey> = GEMS
            .iter()
            .map(|(gem, _)| r.block_key(&teleporter_id(gem)))
            .collect();
        let charms: Vec<ItemKey> = GEMS
            .iter()
            .map(|(gem, _)| r.item_key(&charm_id(gem)))
            .collect();
        for block in &teleporters {
            r.block_part(*block, TeleporterPart);
        }
        for item in &charms {
            r.item_part(*item, CharmItem);
        }
        let linker = r.item_key(LINKER);
        r.item_part(linker, LinkerItem);
        let _ = KEYS.set(Keys {
            teleporters,
            charms,
        });
    }

    fn component_types(&self, _: &RegistrationContext, event: &mut ComponentTypesEvent<'_>) {
        event.types.push(BOUND.id().to_string());
    }

    fn blocks(&self, _: &RegistrationContext, event: &mut BlocksEvent<'_>) {
        for decl in gems::blocks() {
            let mut block = Block::new(decl.id, decl.template).with_item(true);
            block.properties.light = decl.light;
            event.blocks.push(block);
        }
    }

    fn items(&self, _: &RegistrationContext, event: &mut ItemsEvent<'_>) {
        for decl in gems::items() {
            let mut item = Item::new(decl.id, decl.template);
            item.max_stack_size = decl.max_stack_size;
            event.items.push(item);
        }
    }

    /// A charm held to the end warps its player to the bound teleporter.
    fn living_entity_use_item_finish(
        &self,
        host: HostSide,
        event: &mut LivingEntityUseItemFinishEvent<'_>,
    ) {
        let HostSide::Server(host) = host else {
            return;
        };
        if !is_charm(event.stack.item()) {
            return;
        }
        let (Some(to), Some(id)) = (event.stack.get(&BOUND), event.entity) else {
            return;
        };
        let Some(entity) = u32::try_from(id).ok().and_then(|id| host.entity(id)) else {
            return;
        };
        let Some(player) = entity.uuid(host).and_then(|uuid| host.player(uuid)) else {
            return;
        };
        if !warp(host, player, &to) {
            tell(host, player, "silentgems.teleporter.missing");
        }
    }
}
