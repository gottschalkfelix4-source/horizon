//! MineColonies for Horizon, first slice: founding a colony with a Town Hall, the chunks it
//! claims, and the protection of those chunks.
//!
//! Written from MineColonies' observed behaviour and public description (its wiki), never from
//! its code (docs/modpacks.md, "Licences"). What is ported and what still has to be checked
//! against the NeoForge original is in README.md.
//!
//! How it works: a level's colonies are kept in the level attachment `minecolonies:colonies`.
//! Placing a Town Hall (`minecolonies:blockhuttownhall`) founds a colony owned by the placer,
//! unless they already own one here or the colony's claim would come too close to another's;
//! the placement is refused then. A colony claims the square of chunks around its Town Hall's
//! chunk. In a claimed chunk only the colony's owner and officers may place or break blocks.
//! `/minecolonies colony addofficer|removeofficer|delete` manage a colony.
use std::sync::OnceLock;

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockKey, BreakBlockEvent,
    CommandArgumentValue, CommandSource, Compound, EntityPlaceEvent, Flow, Holder, Hooks, ItemPart,
    OnBlockPlacedByEvent, PlayerRef, RegisterCommandsEvent, Registrar, RegistrationContext,
    RunModCommandEvent, ServerHost, Tag, Text,
};

/// The chunks a new colony claims around its Town Hall's chunk, on each side (a square of
/// `2 * INITIAL_RADIUS + 1` chunks). From memory (`initialColonySize`): to check.
pub const INITIAL_RADIUS: i32 = 4;
/// The fewest unclaimed chunks between two colonies' claims. From memory (`minColonyDistance`):
/// to check.
pub const MIN_GAP: i32 = 8;

/// One colony.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Colony {
    pub id: i32,
    pub owner: u128,
    pub officers: Vec<u128>,
    /// The Town Hall's position.
    pub hall: [i32; 3],
    /// The chunks it claims on each side of the Town Hall's chunk.
    pub radius: i32,
}

impl Colony {
    fn center(&self) -> [i32; 2] {
        chunk_of(self.hall)
    }
    /// Whether the colony claims chunk `chunk`.
    pub fn claims(&self, chunk: [i32; 2]) -> bool {
        chebyshev(self.center(), chunk) <= self.radius
    }
    /// Whether `player` may build and break in the colony's chunks.
    pub fn is_member(&self, player: u128) -> bool {
        self.owner == player || self.officers.contains(&player)
    }
}

fn chunk_of(pos: [i32; 3]) -> [i32; 2] {
    [pos[0] >> 4, pos[2] >> 4]
}

fn chebyshev(a: [i32; 2], b: [i32; 2]) -> i32 {
    (a[0] - b[0]).abs().max((a[1] - b[1]).abs())
}

/// Why a Town Hall founds no colony.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    AlreadyOwner,
    TooClose(i32),
}

/// A level's colonies.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Colonies {
    pub next_id: i32,
    pub list: Vec<Colony>,
}

impl Colonies {
    /// The colony claiming the chunk `pos` is in.
    pub fn claim_at(&self, pos: [i32; 3]) -> Option<&Colony> {
        let chunk = chunk_of(pos);
        self.list.iter().find(|c| c.claims(chunk))
    }

    /// Whether `player` may change the block at `pos`: unclaimed land, or their colony's.
    pub fn may_build(&self, player: u128, pos: [i32; 3]) -> Result<(), i32> {
        match self.claim_at(pos) {
            Some(colony) if !colony.is_member(player) => Err(colony.id),
            _ => Ok(()),
        }
    }

    /// Whether `owner` may found a colony with a Town Hall at `hall`.
    pub fn can_found(&self, owner: u128, hall: [i32; 3]) -> Result<(), Refusal> {
        if self.list.iter().any(|c| c.owner == owner) {
            return Err(Refusal::AlreadyOwner);
        }
        let center = chunk_of(hall);
        match self
            .list
            .iter()
            .find(|c| chebyshev(c.center(), center) <= c.radius + INITIAL_RADIUS + MIN_GAP)
        {
            Some(c) => Err(Refusal::TooClose(c.id)),
            None => Ok(()),
        }
    }

    /// Founds a colony: its id.
    pub fn found(&mut self, owner: u128, hall: [i32; 3]) -> Result<i32, Refusal> {
        self.can_found(owner, hall)?;
        self.next_id = self.next_id.max(1);
        let id = self.next_id;
        self.next_id += 1;
        self.list.push(Colony {
            id,
            owner,
            officers: Vec::new(),
            hall,
            radius: INITIAL_RADIUS,
        });
        Ok(id)
    }
}

static COLONIES: AttachmentType<Colonies> = AttachmentType {
    namespace: "minecolonies",
    name: "colonies",
    default: Colonies::default,
    codec: Some(AttachmentCodec {
        encode: encode_colonies,
        decode: decode_colonies,
    }),
    copy_on_death: false,
    sync: false,
};

fn uuid_ints(uuid: u128) -> [i32; 4] {
    std::array::from_fn(|i| (uuid >> (96 - 32 * i)) as u32 as i32)
}

fn uuid_of(ints: &[i32]) -> u128 {
    ints.iter()
        .fold(0u128, |acc, i| (acc << 32) | u128::from(*i as u32))
}

fn encode_colonies(colonies: &Colonies) -> Result<Compound, AttachmentError> {
    let list = colonies
        .list
        .iter()
        .map(|c| {
            let officers = c.officers.iter().flat_map(|o| uuid_ints(*o)).collect();
            Tag::Compound(Compound::from([
                ("id".into(), Tag::Int(c.id)),
                ("owner".into(), Tag::IntArray(uuid_ints(c.owner).to_vec())),
                ("officers".into(), Tag::IntArray(officers)),
                ("hall".into(), Tag::IntArray(c.hall.to_vec())),
                ("radius".into(), Tag::Int(c.radius)),
            ]))
        })
        .collect();
    Ok(Compound::from([
        ("next_id".into(), Tag::Int(colonies.next_id)),
        ("colonies".into(), Tag::List(list)),
    ]))
}

fn decode_colonies(data: &Compound) -> Result<Colonies, AttachmentError> {
    let (Some(Tag::Int(next_id)), Some(Tag::List(list))) =
        (data.get(&"next_id".into()), data.get(&"colonies".into()))
    else {
        return Err(AttachmentError::Decode);
    };
    let list = list
        .iter()
        .map(|tag| {
            let Tag::Compound(c) = tag else {
                return Err(AttachmentError::Decode);
            };
            let get = |key: &str| c.get(&key.into());
            match (
                get("id"),
                get("owner"),
                get("officers"),
                get("hall"),
                get("radius"),
            ) {
                (
                    Some(Tag::Int(id)),
                    Some(Tag::IntArray(owner)),
                    Some(Tag::IntArray(officers)),
                    Some(Tag::IntArray(hall)),
                    Some(Tag::Int(radius)),
                ) if owner.len() == 4 && officers.len() % 4 == 0 && hall.len() == 3 => Ok(Colony {
                    id: *id,
                    owner: uuid_of(owner),
                    officers: officers.chunks_exact(4).map(uuid_of).collect(),
                    hall: [hall[0], hall[1], hall[2]],
                    radius: *radius,
                }),
                _ => Err(AttachmentError::Decode),
            }
        })
        .collect::<Result<_, _>>()?;
    Ok(Colonies {
        next_id: *next_id,
        list,
    })
}

/// `minecolonies:blockhuttownhall`, known once registration ran.
static TOWN_HALL: OnceLock<BlockKey> = OnceLock::new();

fn tell(host: &mut dyn ServerHost, player: PlayerRef, key: &str, id: Option<i32>) {
    let args = id
        .map(|id| Text::literal(id.to_string()))
        .into_iter()
        .collect();
    let _ = player.tell(host, Text::translate(key, args));
}

fn refusal_key(refusal: Refusal) -> (&'static str, Option<i32>) {
    match refusal {
        Refusal::AlreadyOwner => ("minecolonies.horizon.colony.already_owner", None),
        Refusal::TooClose(id) => ("minecolonies.horizon.colony.too_close", Some(id)),
    }
}

/// The Town Hall's item: placing it founds the colony.
struct TownHallItem;

#[mod_api::horizon_part]
impl ItemPart for TownHallItem {
    fn on_block_placed_by(&self, host: &mut dyn ServerHost, event: &mut OnBlockPlacedByEvent<'_>) {
        let Some(player) = event.placer else {
            return;
        };
        if TOWN_HALL.get() != Some(&event.state.block()) {
            return;
        }
        let Ok(mut colonies) = COLONIES.get(host.attachments(), Holder::Level) else {
            return;
        };
        // `entity_place` already refused a placement that founds nothing.
        if let Ok(id) = colonies.found(player.uuid(), event.pos) {
            let _ = COLONIES.set(host.attachments(), Holder::Level, colonies);
            tell(
                host,
                player,
                "minecolonies.horizon.colony.founded",
                Some(id),
            );
        }
    }
}

#[derive(Default)]
struct Mod;

impl Mod {
    /// `/minecolonies colony addofficer|removeofficer|delete <colony> [player]`.
    fn command(
        colonies: &mut Colonies,
        event: &RunModCommandEvent<'_>,
    ) -> (&'static str, Option<i32>) {
        let Some(CommandArgumentValue::Integer(id)) = event.command.argument("colony") else {
            return ("minecolonies.horizon.command.no_colony", None);
        };
        let id = *id;
        let (caller, operator) = match event.source {
            CommandSource::Player { player, permission } => {
                (Some(player.uuid()), permission.level() >= 2)
            }
            source => (None, source.permission().level() >= 2),
        };
        let Some(index) = colonies.list.iter().position(|c| c.id == id) else {
            return ("minecolonies.horizon.command.no_colony", Some(id));
        };
        if !operator && caller != Some(colonies.list[index].owner) {
            return ("minecolonies.horizon.command.not_owner", None);
        }
        let target = match event.command.argument("player") {
            Some(CommandArgumentValue::Player(p)) => Some(p.uuid()),
            _ => None,
        };
        let colony = &mut colonies.list[index];
        match (event.command.literals.get(2).map(String::as_str), target) {
            (Some("addofficer"), Some(t)) => {
                if !colony.officers.contains(&t) {
                    colony.officers.push(t);
                }
                ("minecolonies.horizon.command.officer_added", Some(id))
            }
            (Some("removeofficer"), Some(t)) => {
                colony.officers.retain(|o| *o != t);
                ("minecolonies.horizon.command.officer_removed", Some(id))
            }
            (Some("delete"), _) => {
                colonies.list.remove(index);
                ("minecolonies.horizon.command.deleted", Some(id))
            }
            _ => ("minecolonies.horizon.command.no_colony", Some(id)),
        }
    }
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let _ = TOWN_HALL.set(r.block_key("minecolonies:blockhuttownhall"));
        let item = r.item_key("minecolonies:blockhuttownhall");
        r.item_part(item, TownHallItem);
    }

    /// Refuses a Town Hall that would found nothing, and a player's block in another's colony.
    fn entity_place(&self, host: &mut dyn ServerHost, event: &mut EntityPlaceEvent) -> Flow {
        let Some(player) = host.players().acting_player() else {
            return Flow::Continue;
        };
        if event.placer != Some(player.entity()) {
            return Flow::Continue;
        }
        let Ok(colonies) = COLONIES.get(host.attachments(), Holder::Level) else {
            return Flow::Continue;
        };
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        let refused = if TOWN_HALL.get() == Some(&event.state.block()) {
            colonies
                .can_found(player.uuid(), pos)
                .err()
                .map(refusal_key)
        } else {
            colonies
                .may_build(player.uuid(), pos)
                .err()
                .map(|id| ("minecolonies.horizon.colony.protected", Some(id)))
        };
        match refused {
            Some((key, id)) => {
                tell(host, player, key, id);
                Flow::Cancel
            }
            None => Flow::Continue,
        }
    }

    /// Refuses a player's break in another's colony.
    fn break_block(&self, host: &mut dyn ServerHost, event: &mut BreakBlockEvent<'_>) -> Flow {
        let Some(player) = host.players().acting_player() else {
            return Flow::Continue;
        };
        let Ok(colonies) = COLONIES.get(host.attachments(), Holder::Level) else {
            return Flow::Continue;
        };
        match colonies.may_build(player.uuid(), event.pos) {
            Ok(()) => Flow::Continue,
            Err(id) => {
                tell(
                    host,
                    player,
                    "minecolonies.horizon.colony.protected",
                    Some(id),
                );
                Flow::Cancel
            }
        }
    }

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.extend([
            mod_api::Command::new(
                "minecolonies colony addofficer <colony:integer> <player:player>",
            ),
            mod_api::Command::new(
                "minecolonies colony removeofficer <colony:integer> <player:player>",
            ),
            mod_api::Command::new("minecolonies colony delete <colony:integer>"),
        ]);
    }

    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        if event.command.literals.first().map(String::as_str) != Some("minecolonies") {
            return Flow::Continue;
        }
        let Ok(mut colonies) = COLONIES.get(host.attachments(), Holder::Level) else {
            return Flow::Cancel;
        };
        let before = colonies.clone();
        let (key, id) = Self::command(&mut colonies, event);
        if colonies != before {
            let _ = COLONIES.set(host.attachments(), Holder::Level, colonies);
        }
        let args = id
            .map(|id| Text::literal(id.to_string()))
            .into_iter()
            .collect();
        let _ = host.commands().feedback(Text::translate(key, args));
        Flow::Cancel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: u128 = 0xA11CE;
    const BOB: u128 = 0xB0B;

    #[test]
    fn a_colony_claims_the_square_of_chunks_around_its_hall() {
        let mut colonies = Colonies::default();
        // Chunk (0, 0); the claim runs from chunk -4 to chunk 4 on each axis.
        assert_eq!(colonies.found(ALICE, [8, 64, 8]), Ok(1));
        assert!(colonies.claim_at([4 * 16 + 15, 0, -4 * 16]).is_some());
        assert!(colonies.claim_at([5 * 16, 0, 0]).is_none());
        assert!(colonies.claim_at([0, 0, -5 * 16 - 1]).is_none());
    }

    #[test]
    fn only_owner_and_officers_build_in_a_colony() {
        let mut colonies = Colonies::default();
        let id = colonies.found(ALICE, [0, 64, 0]).unwrap();
        assert_eq!(colonies.may_build(BOB, [10, 64, 10]), Err(id));
        assert_eq!(colonies.may_build(ALICE, [10, 64, 10]), Ok(()));
        assert_eq!(colonies.may_build(BOB, [16 * 20, 64, 0]), Ok(()));
        colonies.list[0].officers.push(BOB);
        assert_eq!(colonies.may_build(BOB, [10, 64, 10]), Ok(()));
    }

    #[test]
    fn a_second_colony_keeps_its_distance_and_owners_found_once() {
        let mut colonies = Colonies::default();
        let first = colonies.found(ALICE, [0, 64, 0]).unwrap();
        assert_eq!(
            colonies.found(ALICE, [16 * 100, 64, 0]),
            Err(Refusal::AlreadyOwner)
        );
        // Claims of 4 chunks each side plus a gap of 8: centres 16 chunks apart are too close,
        // 17 are fine.
        assert_eq!(
            colonies.found(BOB, [16 * 16, 64, 0]),
            Err(Refusal::TooClose(first))
        );
        assert_eq!(colonies.found(BOB, [16 * 17, 64, 0]), Ok(2));
    }

    #[test]
    fn colonies_round_trip_through_their_saved_form() {
        let mut colonies = Colonies::default();
        colonies.found(u128::MAX - 5, [-100, 70, 3000]).unwrap();
        colonies.list[0].officers.push(1 << 127);
        let saved = encode_colonies(&colonies).unwrap();
        assert_eq!(decode_colonies(&saved).unwrap(), colonies);
    }
}
