//! Super Factory Manager for Horizon: the Factory Manager runs an SFML program that moves items
//! between labelled inventories reachable through its cables.
//!
//! Written from SFM's public documentation and observed behaviour, never from its code
//! (docs/modpacks.md, "Licences"). What is ported and what still has to be checked against the
//! NeoForge original is in README.md.
//!
//! How it works: each level keeps its managers in a level attachment (`sfm:managers`): the
//! manager's position, its program's text and its labels (name to inventory position), set with
//! `/sfm program`, `/sfm label` and `/sfm unlabel` (the original edits them in the manager's
//! screen and with the Label Gun). After each level tick, every manager whose trigger interval
//! divides the level's game time runs that trigger: its inventories are the labelled positions
//! touching the manager or a cable connected to it, reached through the item storage
//! capability, each move in a transaction of its own.
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Mutex, OnceLock};

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, BlockKey, CommandArgumentValue, Compound,
    Direction, Flow, Holder, Hooks, HostSide, HostTransaction, LevelRef, LevelTickPostEvent,
    MAX_STORAGE_PAGE, RegisterCommandsEvent, Registrar, RegistrationContext, RunModCommandEvent,
    STORAGE, ServerHost, StorageError, StoragePage, Tag, Text, transfer,
};

pub mod sfml;

use sfml::{Endpoint, Program, Side};

/// The most cables one network walk follows, so a huge cable field stays a bounded cost.
pub const MAX_CABLES: usize = 8192;

/// One placed Factory Manager.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manager {
    pub pos: [i32; 3],
    pub program: String,
    pub labels: Vec<(String, [i32; 3])>,
}

const NEIGHBOURS: [[i32; 3]; 6] = [
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
    [-1, 0, 0],
    [1, 0, 0],
];

fn offset(pos: [i32; 3], d: [i32; 3]) -> [i32; 3] {
    [pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]]
}

/// The manager and every cable connected to it through faces, at most `max` cables.
pub fn network(
    manager: [i32; 3],
    is_cable: &dyn Fn([i32; 3]) -> bool,
    max: usize,
) -> HashSet<[i32; 3]> {
    let mut seen = HashSet::from([manager]);
    let mut queue = VecDeque::from([manager]);
    while let Some(at) = queue.pop_front() {
        for d in NEIGHBOURS {
            let next = offset(at, d);
            if seen.len() > max || seen.contains(&next) || !is_cable(next) {
                continue;
            }
            seen.insert(next);
            queue.push_back(next);
        }
    }
    seen
}

/// Whether an inventory at `pos` touches the network.
pub fn on_network(net: &HashSet<[i32; 3]>, pos: [i32; 3]) -> bool {
    !net.contains(&pos) && NEIGHBOURS.iter().any(|d| net.contains(&offset(pos, *d)))
}

static MANAGERS: AttachmentType<Vec<Manager>> = AttachmentType {
    namespace: "sfm",
    name: "managers",
    default: Vec::new,
    codec: Some(AttachmentCodec {
        encode: encode_managers,
        decode: decode_managers,
    }),
    copy_on_death: false,
    sync: false,
};

fn encode_managers(managers: &Vec<Manager>) -> Result<Compound, AttachmentError> {
    let list = managers
        .iter()
        .map(|m| {
            let labels = m
                .labels
                .iter()
                .map(|(name, pos)| {
                    Tag::Compound(Compound::from([
                        ("name".into(), Tag::String(name.as_str().into())),
                        ("pos".into(), Tag::IntArray(pos.to_vec())),
                    ]))
                })
                .collect();
            Tag::Compound(Compound::from([
                ("pos".into(), Tag::IntArray(m.pos.to_vec())),
                ("program".into(), Tag::String(m.program.as_str().into())),
                ("labels".into(), Tag::List(labels)),
            ]))
        })
        .collect();
    Ok([("managers".into(), Tag::List(list))].into())
}

fn pos_of(tag: Option<&Tag>) -> Result<[i32; 3], AttachmentError> {
    match tag {
        Some(Tag::IntArray(v)) if v.len() == 3 => Ok([v[0], v[1], v[2]]),
        _ => Err(AttachmentError::Decode),
    }
}

fn string_of(tag: Option<&Tag>) -> Result<String, AttachmentError> {
    match tag {
        Some(Tag::String(s)) => Ok(s.to_string_lossy()),
        _ => Err(AttachmentError::Decode),
    }
}

fn decode_managers(data: &Compound) -> Result<Vec<Manager>, AttachmentError> {
    let Some(Tag::List(list)) = data.get(&"managers".into()) else {
        return Err(AttachmentError::Decode);
    };
    list.iter()
        .map(|tag| {
            let Tag::Compound(m) = tag else {
                return Err(AttachmentError::Decode);
            };
            let labels = match m.get(&"labels".into()) {
                Some(Tag::List(labels)) => labels
                    .iter()
                    .map(|l| match l {
                        Tag::Compound(l) => Ok((
                            string_of(l.get(&"name".into()))?,
                            pos_of(l.get(&"pos".into()))?,
                        )),
                        _ => Err(AttachmentError::Decode),
                    })
                    .collect::<Result<_, _>>()?,
                _ => Vec::new(),
            };
            Ok(Manager {
                pos: pos_of(m.get(&"pos".into()))?,
                program: string_of(m.get(&"program".into()))?,
                labels,
            })
        })
        .collect()
}

/// `sfm:manager` and `sfm:cable`, known once registration ran.
static KEYS: OnceLock<[BlockKey; 2]> = OnceLock::new();

/// Programs already read, by their text: a manager's program is read once, not every tick.
static PROGRAMS: Mutex<Option<HashMap<String, Option<Program>>>> = Mutex::new(None);

fn compiled(text: &str) -> Option<Program> {
    let mut cache = PROGRAMS.lock().ok()?;
    let cache = cache.get_or_insert_with(HashMap::new);
    if cache.len() > 256 {
        cache.clear();
    }
    cache
        .entry(text.to_owned())
        .or_insert_with(|| sfml::parse(text).ok())
        .clone()
}

fn direction(side: Option<Side>) -> Option<Direction> {
    Some(match side? {
        Side::Top => Direction::Up,
        Side::Bottom => Direction::Down,
        Side::North => Direction::North,
        Side::South => Direction::South,
        Side::East => Direction::East,
        Side::West => Direction::West,
    })
}

/// Moves up to `max` items `filter` accepts from one inventory to another, in its own
/// transaction: how many went.
fn move_items(
    host: &mut dyn ServerHost,
    level: LevelRef,
    from: Endpoint,
    to: Endpoint,
    filter: &dyn Fn(&str) -> bool,
    max: u64,
) -> u64 {
    let Some(src) = level.capability(host, &STORAGE, from.0, direction(from.1)) else {
        return 0;
    };
    let Some(dst) = level.capability(host, &STORAGE, to.0, direction(to.1)) else {
        return 0;
    };
    host.transaction(|host, tx| -> Result<u64, StorageError> {
        let mut page = StoragePage::new(MAX_STORAGE_PAGE)?;
        let (mut cursor, mut kinds) = (Some(0), Vec::new());
        while let Some(at) = cursor {
            cursor = src.page(host, at, &mut page)?;
            kinds.extend(page.entries().iter().map(|(kind, _)| kind.clone()));
        }
        let mut moved = 0;
        for kind in kinds {
            if moved == max {
                break;
            }
            if filter(&kind.item().id(&*host)) {
                moved += transfer(host, &src, &dst, &kind, max - moved, tx)?;
            }
        }
        Ok(moved)
    })
    .unwrap_or(0)
}

fn run_manager(host: &mut dyn ServerHost, level: LevelRef, manager: &Manager, time: i64) {
    let Some(program) = compiled(&manager.program) else {
        return;
    };
    let due: Vec<_> = program
        .triggers
        .iter()
        .filter(|t| time.rem_euclid(i64::from(t.interval.max(1))) == 0)
        .collect();
    if due.is_empty() {
        return;
    }
    let Some(&[_, cable]) = KEYS.get() else {
        return;
    };
    let net = network(
        manager.pos,
        &|pos| level.block(&*host, pos) == Some(cable),
        MAX_CABLES,
    );
    let resolve = |label: &str| -> Vec<[i32; 3]> {
        manager
            .labels
            .iter()
            .filter(|(name, pos)| name == label && on_network(&net, *pos))
            .map(|(_, pos)| *pos)
            .collect()
    };
    for trigger in due {
        sfml::run(trigger, &resolve, &mut |from, to, filter, max| {
            move_items(host, level, from, to, filter, max)
        });
    }
}

#[derive(Default)]
struct Mod;

fn feedback(host: &mut dyn ServerHost, key: &str, args: Vec<Text>) {
    let _ = host.commands().feedback(Text::translate(key, args));
}

impl Mod {
    /// `/sfm program|label|unlabel`: answers the feedback key, the managers changed in place.
    fn command(
        host: &mut dyn ServerHost,
        level: LevelRef,
        event: &RunModCommandEvent<'_>,
        managers: &mut Vec<Manager>,
    ) -> Result<&'static str, (&'static str, Vec<Text>)> {
        let Some(CommandArgumentValue::BlockPos(pos)) = event.command.argument("manager") else {
            return Err(("sfm.horizon.command.no_manager", Vec::new()));
        };
        let Some(&[manager_key, _]) = KEYS.get() else {
            return Err(("sfm.horizon.command.no_manager", Vec::new()));
        };
        if level.block(&*host, *pos) != Some(manager_key) {
            return Err(("sfm.horizon.command.no_manager", Vec::new()));
        }
        let index = match managers.iter().position(|m| m.pos == *pos) {
            Some(i) => i,
            None => {
                managers.push(Manager {
                    pos: *pos,
                    ..Manager::default()
                });
                managers.len() - 1
            }
        };
        let manager = &mut managers[index];
        let word = |name: &str| match event.command.argument(name) {
            Some(CommandArgumentValue::String(s)) => s.clone(),
            _ => String::new(),
        };
        match event.command.literals.get(1).map(String::as_str) {
            Some("program") => {
                let code = word("code");
                if let Err(e) = sfml::parse(&code) {
                    return Err((e.key, vec![Text::literal(e.word.to_string())]));
                }
                manager.program = code;
                Ok("sfm.horizon.command.program_saved")
            }
            Some("label") => {
                let Some(CommandArgumentValue::BlockPos(target)) = event.command.argument("target")
                else {
                    return Err(("sfm.horizon.command.no_manager", Vec::new()));
                };
                let name = word("name");
                if !manager.labels.iter().any(|l| l.0 == name && l.1 == *target) {
                    manager.labels.push((name, *target));
                }
                Ok("sfm.horizon.command.labelled")
            }
            Some("unlabel") => {
                let name = word("name");
                manager.labels.retain(|l| l.0 != name);
                Ok("sfm.horizon.command.unlabelled")
            }
            _ => Err(("sfm.horizon.command.no_manager", Vec::new())),
        }
    }
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let _ = KEYS.set([r.block_key("sfm:manager"), r.block_key("sfm:cable")]);
    }

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.extend([
            mod_api::Command::new("sfm program <manager:block_pos> <code:text>"),
            mod_api::Command::new("sfm label <manager:block_pos> <name:word> <target:block_pos>"),
            mod_api::Command::new("sfm unlabel <manager:block_pos> <name:word>"),
        ]);
    }

    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        if event.command.literals.first().map(String::as_str) != Some("sfm") {
            return Flow::Continue;
        }
        let Some(level) = event
            .command
            .dimension
            .id(&*host)
            .and_then(|id| host.level(&id))
        else {
            return Flow::Cancel;
        };
        let Ok(mut managers) = MANAGERS.get(host.attachments(), Holder::Level) else {
            return Flow::Cancel;
        };
        match Self::command(host, level, event, &mut managers) {
            Ok(key) => {
                let _ = MANAGERS.set(host.attachments(), Holder::Level, managers);
                feedback(host, key, Vec::new());
            }
            Err((key, args)) => feedback(host, key, args),
        }
        Flow::Cancel
    }

    fn level_tick_post(&self, host: HostSide, event: &mut LevelTickPostEvent) {
        let HostSide::Server(host) = host else {
            return;
        };
        let Ok(mut managers) = MANAGERS.get(host.attachments(), Holder::Level) else {
            return;
        };
        if managers.is_empty() {
            return;
        }
        let (Some(time), Some(&[manager_key, _])) = (event.level.game_time(&*host), KEYS.get())
        else {
            return;
        };
        let before = managers.len();
        // A manager whose block has gone (broken, exploded) is forgotten; an unloaded one waits.
        managers.retain(|m| {
            event
                .level
                .block(&*host, m.pos)
                .is_none_or(|b| b == manager_key)
        });
        if managers.len() != before {
            let _ = MANAGERS.set(host.attachments(), Holder::Level, managers.clone());
        }
        for manager in &managers {
            if event.level.block(&*host, manager.pos) == Some(manager_key) {
                run_manager(host, event.level, manager, time);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sfml::*;
    use super::*;

    #[test]
    fn a_program_reads_items_limits_labels_and_sides() {
        let program = parse(
            "name \"ores\" every 2 seconds do input 64 iron_ore, gold_ore from a, b \
             output to furnace top side end",
        )
        .unwrap();
        assert_eq!(program.name.as_deref(), Some("ores"));
        let t = &program.triggers[0];
        assert_eq!(t.interval, 40);
        assert_eq!(t.statements[0].labels, ["a", "b"]);
        assert_eq!(t.statements[0].entries[0].limit, Some(64));
        assert_eq!(t.statements[0].entries[1].limit, None);
        assert_eq!(t.statements[1].side, Some(Side::Top));
        assert!(t.statements[1].entries[0].matches("mod:anything"));
    }

    #[test]
    fn mistakes_are_refused_at_their_word() {
        let short = parse("EVERY 5 TICKS DO INPUT FROM a OUTPUT TO b END").unwrap_err();
        assert_eq!(short.key, "sfm.horizon.error.interval_too_short");
        assert_eq!(short.word, 2);
        let open = parse("EVERY 20 TICKS DO INPUT FROM a").unwrap_err();
        assert_eq!(open.key, "sfm.horizon.error.expected_end");
    }

    #[test]
    fn patterns_match_with_or_without_a_namespace() {
        let entry = |p: &str| Entry {
            limit: None,
            pattern: p.into(),
        };
        assert!(entry("iron_ingot").matches("minecraft:iron_ingot"));
        assert!(entry("*ingot").matches("mekanism:osmium_ingot"));
        assert!(!entry("minecraft:*ingot").matches("mekanism:osmium_ingot"));
        assert!(!entry("iron").matches("minecraft:iron_ingot"));
    }

    #[test]
    fn outputs_share_an_inputs_limit() {
        let program =
            parse("EVERY 20 TICKS DO INPUT 10 FROM chest OUTPUT 6 TO left OUTPUT TO right END")
                .unwrap();
        let resolve = |label: &str| match label {
            "chest" => vec![[0, 0, 0]],
            "left" => vec![[1, 0, 0]],
            "right" => vec![[2, 0, 0]],
            _ => vec![],
        };
        let mut moves = Vec::new();
        // The chest holds 100 items: each move takes all it is allowed.
        let total = run(&program.triggers[0], &resolve, &mut |from, to, _, max| {
            let went = max.min(100);
            moves.push((from.0, to.0, went));
            went
        });
        assert_eq!(
            moves,
            [([0, 0, 0], [1, 0, 0], 6), ([0, 0, 0], [2, 0, 0], 4)]
        );
        assert_eq!(total, 10);
    }

    #[test]
    fn only_inventories_touching_connected_cables_are_reached() {
        let cables: HashSet<[i32; 3]> = [[1, 0, 0], [2, 0, 0], [9, 0, 0]].into();
        let net = network([0, 0, 0], &|p| cables.contains(&p), MAX_CABLES);
        assert!(on_network(&net, [3, 0, 0]));
        assert!(on_network(&net, [0, 1, 0]));
        assert!(!on_network(&net, [10, 0, 0]));
        assert!(!on_network(&net, [1, 0, 0]));
    }
}
