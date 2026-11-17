//! InControl for Horizon: pack authors control mob spawns and experience drops with rule files
//! in `config/incontrol/` (`spawn.json`, `experience.json`).
//!
//! Written from InControl's public documentation (its wiki's rule format) and observed
//! behaviour, never from its code (docs/modpacks.md, "Licences"). Which conditions and results
//! are read, and what is left, is in README.md; a rule using anything else is ignored and
//! reported by `/incontrol reload`, never half-applied.
use std::cell::OnceCell;
use std::sync::RwLock;

use mod_api::{
    Command, ConfigPath, EntityJoinLevelEvent, FinalizeSpawnEvent, Flow, Hooks, Host, LevelRef,
    LightLayer, LivingExperienceDropEvent, MobCategory, MobSpawnPositionCheckEvent,
    RegisterCommandsEvent, RunModCommandEvent, ServerHost, ServerStartedEvent, SpawnCheck,
    SpawnReason, Text,
};

pub mod rules;
mod text;

use rules::{CountOf, Facts, Outcome, Parsed, Reason, SpawnRule, When, XpRule};
use text::{lit, tr};

#[derive(Default)]
struct Loaded {
    spawn: Vec<(usize, SpawnRule)>,
    experience: Vec<(usize, XpRule)>,
    /// What `/incontrol reload` reports: problems with files and ignored rules.
    report: Vec<Text>,
}

static RULES: RwLock<Option<Loaded>> = RwLock::new(None);

fn read_file<T>(
    host: &dyn ServerHost,
    name: &str,
    parse: fn(&str) -> Result<Parsed<T>, String>,
    report: &mut Vec<Text>,
) -> Vec<(usize, T)> {
    let path = format!("incontrol/{name}");
    let Ok(file) = ConfigPath::new(&path) else {
        return Vec::new();
    };
    // A missing file is no rules, as in the original.
    let Ok(bytes) = host.config_files().read(&file) else {
        return Vec::new();
    };
    match parse(&String::from_utf8_lossy(&bytes)) {
        Ok(parsed) => {
            for (index, reason) in parsed.skipped {
                report.push(tr(
                    "incontrol.skipped",
                    vec![lit(index + 1), lit(name), lit(reason)],
                ));
            }
            parsed.rules
        }
        Err(error) => {
            report.push(tr("incontrol.file_error", vec![lit(name), lit(error)]));
            Vec::new()
        }
    }
}

fn load(host: &dyn ServerHost) {
    let mut report = Vec::new();
    let spawn = read_file(host, "spawn.json", rules::parse_spawn, &mut report);
    let experience = read_file(
        host,
        "experience.json",
        rules::parse_experience,
        &mut report,
    );
    if let Ok(mut slot) = RULES.write() {
        *slot = Some(Loaded {
            spawn,
            experience,
            report,
        });
    }
}

fn hostile(host: &dyn Host, kind: &str) -> Option<bool> {
    match host.entity_type_category(kind)? {
        MobCategory::Monster => Some(true),
        MobCategory::Misc => None,
        _ => Some(false),
    }
}

fn reason(reason: SpawnReason) -> Reason {
    match reason {
        SpawnReason::Natural | SpawnReason::ChunkGeneration => Reason::Natural,
        SpawnReason::Spawner | SpawnReason::TrialSpawner => Reason::Spawner,
        _ => Reason::Other,
    }
}

/// The world as one check sees it, read from the host only as conditions ask.
struct World<'a> {
    host: &'a dyn Host,
    level: Option<LevelRef>,
    kind: &'a str,
    dimension: &'a str,
    pos: [f64; 3],
    reason: Reason,
    by_player: Option<bool>,
    kinds: OnceCell<Vec<String>>,
}

impl<'a> World<'a> {
    fn new(host: &'a dyn Host, kind: &'a str, dimension: &'a str, pos: [f64; 3]) -> Self {
        Self {
            host,
            level: host.level(dimension),
            kind,
            dimension,
            pos,
            reason: Reason::Other,
            by_player: None,
            kinds: OnceCell::new(),
        }
    }
    fn block(&self) -> [i32; 3] {
        self.pos.map(|c| c.floor() as i32)
    }
    fn kinds(&self) -> &[String] {
        self.kinds.get_or_init(|| {
            self.level
                .and_then(|level| level.all_entities(self.host))
                .unwrap_or_default()
                .into_iter()
                .filter_map(|e| e.kind(self.host).map(|k| k.id(self.host)))
                .collect()
        })
    }
}

impl Facts for World<'_> {
    fn kind(&self) -> &str {
        self.kind
    }
    fn hostile(&self) -> Option<bool> {
        hostile(self.host, self.kind)
    }
    fn dimension(&self) -> &str {
        self.dimension
    }
    fn height(&self) -> f64 {
        self.pos[1]
    }
    fn biome(&self) -> Option<String> {
        self.level?.biome_at(self.host, self.block())
    }
    fn biome_in_tag(&self, tag: &str) -> bool {
        let tag = tag.trim_start_matches('#');
        self.biome().is_some_and(|biome| {
            self.level
                .and_then(|level| level.biome_in_tag(self.host, &biome, tag))
                .unwrap_or(false)
        })
    }
    fn light(&self) -> Option<i32> {
        let level = self.level?;
        let pos = self.block();
        let block = level.brightness(self.host, LightLayer::Block, pos)?;
        let sky = level.brightness(self.host, LightLayer::Sky, pos)?;
        let darken = level.get_sky_darken(self.host).unwrap_or(0);
        Some(block.max(sky - darken))
    }
    fn day_time(&self) -> Option<i64> {
        self.level?.day_time(self.host)
    }
    fn sees_sky(&self) -> Option<bool> {
        self.level?.can_see_sky(self.host, self.block())
    }
    fn raining(&self) -> bool {
        self.level
            .and_then(|l| l.is_raining(self.host))
            .unwrap_or(false)
    }
    fn thundering(&self) -> bool {
        self.level
            .and_then(|l| l.is_thundering(self.host))
            .unwrap_or(false)
    }
    fn difficulty(&self) -> Option<String> {
        Some(self.level?.difficulty(self.host)?.to_ascii_lowercase())
    }
    fn reason(&self) -> Reason {
        self.reason
    }
    fn by_player(&self) -> Option<bool> {
        self.by_player
    }
    fn count(&self, of: &CountOf) -> usize {
        let kinds = self.kinds();
        match of {
            CountOf::ThisMob => kinds.iter().filter(|k| *k == self.kind).count(),
            CountOf::Mobs(mobs) => kinds.iter().filter(|k| mobs.contains(k)).count(),
            CountOf::Mods(mods) => kinds
                .iter()
                .filter(|k| mods.iter().any(|m| k.split(':').next() == Some(m.as_str())))
                .count(),
            CountOf::Hostile => kinds
                .iter()
                .filter(|k| hostile(self.host, k) == Some(true))
                .count(),
            CountOf::Passive => kinds
                .iter()
                .filter(|k| hostile(self.host, k) == Some(false))
                .count(),
            CountOf::All => kinds.len(),
        }
    }
    fn random(&self, rule: usize) -> f64 {
        let time = self.level.and_then(|l| l.game_time(self.host)).unwrap_or(0);
        let kind = self
            .kind
            .bytes()
            .fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(u64::from(b)));
        let [x, y, z] = self.block();
        let pos = (x as u64) ^ ((y as u64) << 21) ^ ((z as u64) << 42);
        rules::draw([time as u64, pos, kind, rule as u64])
    }
}

fn spawn_outcome(world: &World<'_>, when: When) -> Outcome {
    RULES
        .read()
        .ok()
        .and_then(|rules| {
            rules
                .as_ref()
                .map(|r| rules::spawn_outcome(&r.spawn, when, world))
        })
        .unwrap_or(Outcome::Default)
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn server_started(&self, host: &mut dyn ServerHost, _: &mut ServerStartedEvent) {
        load(host);
    }

    /// `"when": "position"` (the default): before the mob exists.
    fn mob_spawn_position_check(
        &self,
        host: &dyn Host,
        event: &mut MobSpawnPositionCheckEvent<'_>,
    ) {
        let mut world = World::new(host, event.kind, event.dimension, event.pos);
        world.level = Some(event.level);
        world.reason = reason(event.reason);
        match spawn_outcome(&world, When::Position) {
            Outcome::Allow => *event.result = SpawnCheck::Succeed,
            Outcome::Deny => *event.result = SpawnCheck::Fail,
            Outcome::Default => {}
        }
    }

    /// `"when": "finalize"`: a denied mob is not added.
    fn finalize_spawn(&self, host: &dyn Host, event: &mut FinalizeSpawnEvent<'_>) -> Flow {
        let mut world = World::new(host, event.kind, event.dimension, event.pos);
        world.reason = reason(event.reason);
        if spawn_outcome(&world, When::Finalize) == Outcome::Deny {
            event.spawn_cancelled = true;
        }
        Flow::Continue
    }

    /// `"when": "onjoin"`: a denied entity does not join (entities loaded from disk are left
    /// alone).
    fn entity_join_level(
        &self,
        host: &mut dyn ServerHost,
        event: &mut EntityJoinLevelEvent<'_>,
    ) -> Flow {
        if event.loaded_from_disk {
            return Flow::Continue;
        }
        let world = World::new(host, event.kind, event.dimension, event.pos);
        if spawn_outcome(&world, When::OnJoin) == Outcome::Deny {
            return Flow::Cancel;
        }
        Flow::Continue
    }

    fn living_experience_drop(
        &self,
        host: &mut dyn ServerHost,
        event: &mut LivingExperienceDropEvent<'_>,
    ) -> Flow {
        let entity = host.entity(event.entity);
        let pos = entity.and_then(|e| e.position(host)).unwrap_or([0.0; 3]);
        let dimension = entity
            .and_then(|e| e.level(host))
            .and_then(|l| l.dimension().id(host))
            .unwrap_or_default();
        let mut world = World::new(host, event.kind, &dimension, pos);
        world.by_player = Some(event.attacking_player.is_some());
        let amount = RULES.read().ok().and_then(|rules| {
            rules
                .as_ref()
                .map(|r| rules::experience(&r.experience, event.amount, &world))
        });
        if let Some(amount) = amount {
            event.amount = amount;
        }
        Flow::Continue
    }

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.push(Command::op("incontrol reload", 2));
    }

    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        if event.command.literals != ["incontrol", "reload"] {
            return Flow::Continue;
        }
        load(host);
        let (counts, report) = RULES
            .read()
            .ok()
            .and_then(|r| {
                r.as_ref()
                    .map(|r| ((r.spawn.len(), r.experience.len()), r.report.clone()))
            })
            .unwrap_or_default();
        let _ = host
            .commands()
            .feedback(tr("incontrol.reloaded", vec![lit(counts.0), lit(counts.1)]));
        for line in report {
            let _ = host.commands().feedback(line);
        }
        Flow::Cancel
    }
}

#[cfg(test)]
mod tests;
