//! InControl's rule files, read and evaluated without the game: `spawn.json` and
//! `experience.json` in `config/incontrol/`. Each file is a JSON array of rules; a rule is a set
//! of conditions (all must hold) and a result. The first rule whose conditions hold decides.
//!
//! A rule with a condition this port does not know yet is kept out (never matches) and reported,
//! so a pack's file never does something its author did not write.
use serde_json::{Map, Value};

/// The spawn stage a spawn rule applies at (`"when"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum When {
    /// The position check before the mob exists (the default).
    Position,
    /// Just before the new mob is set up (`finalizeSpawn`).
    Finalize,
    /// When the entity joins the level.
    OnJoin,
}

/// A spawn rule's answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Allow,
    Deny,
    Default,
}

/// Why a spawn is happening, as far as the rules ask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    Natural,
    Spawner,
    Other,
}

/// What a `mincount`/`maxcount` counts.
#[derive(Clone, Debug, PartialEq)]
pub enum CountOf {
    /// The rule's own mob type (a bare number).
    ThisMob,
    /// These entity types.
    Mobs(Vec<String>),
    /// Every entity whose type's namespace is one of these.
    Mods(Vec<String>),
    Hostile,
    Passive,
    All,
}

/// One `mincount`/`maxcount`.
#[derive(Clone, Debug, PartialEq)]
pub struct Count {
    pub amount: usize,
    pub of: CountOf,
}

/// The conditions shared by both files.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Conditions {
    pub mobs: Option<Vec<String>>,
    pub mods: Option<Vec<String>>,
    pub hostile: Option<bool>,
    pub passive: Option<bool>,
    pub dimensions: Option<Vec<String>>,
    pub biomes: Option<Vec<String>>,
    pub biome_tags: Option<Vec<String>>,
    pub min_height: Option<f64>,
    pub max_height: Option<f64>,
    pub min_light: Option<i32>,
    pub max_light: Option<i32>,
    pub min_time: Option<i64>,
    pub max_time: Option<i64>,
    pub see_sky: Option<bool>,
    pub weather: Option<String>,
    pub difficulty: Option<String>,
    pub random: Option<f64>,
    pub spawner: Option<bool>,
    pub player: Option<bool>,
    pub min_count: Option<Count>,
    pub max_count: Option<Count>,
}

/// What a rule can learn about the world, asked only when a condition needs it.
pub trait Facts {
    fn kind(&self) -> &str;
    /// Whether the mob's category is `monster` (true), another living category (false), or
    /// unknown.
    fn hostile(&self) -> Option<bool>;
    fn dimension(&self) -> &str;
    fn height(&self) -> f64;
    fn biome(&self) -> Option<String>;
    fn biome_in_tag(&self, tag: &str) -> bool;
    fn light(&self) -> Option<i32>;
    fn day_time(&self) -> Option<i64>;
    fn sees_sky(&self) -> Option<bool>;
    fn raining(&self) -> bool;
    fn thundering(&self) -> bool;
    fn difficulty(&self) -> Option<String>;
    fn reason(&self) -> Reason;
    /// Killed by a player (experience rules); None where it does not apply.
    fn by_player(&self) -> Option<bool>;
    fn count(&self, of: &CountOf) -> usize;
    /// A number in [0, 1), fixed for this check and rule.
    fn random(&self, rule: usize) -> f64;
}

/// One rule of `spawn.json`.
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnRule {
    pub when: When,
    pub conditions: Conditions,
    pub outcome: Outcome,
}

/// What an experience rule does to the dropped amount.
#[derive(Clone, Debug, PartialEq)]
pub struct XpChange {
    pub deny: bool,
    pub set: Option<i32>,
    pub multiply: Option<f64>,
    pub add: Option<i32>,
}

impl XpChange {
    /// The amount after the change: deny gives 0, otherwise set, then multiply, then add.
    pub fn apply(&self, amount: i32) -> i32 {
        if self.deny {
            return 0;
        }
        let mut amount = self.set.unwrap_or(amount) as f64;
        if let Some(factor) = self.multiply {
            amount *= factor;
        }
        (amount as i32 + self.add.unwrap_or(0)).max(0)
    }
}

/// One rule of `experience.json`.
#[derive(Clone, Debug, PartialEq)]
pub struct XpRule {
    pub conditions: Conditions,
    pub change: XpChange,
}

/// A file's rules, with the ones left out and why: `(index, reason)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Parsed<T> {
    pub rules: Vec<(usize, T)>,
    pub skipped: Vec<(usize, String)>,
}

fn strings(value: &Value) -> Result<Vec<String>, String> {
    match value {
        Value::String(s) => Ok(vec![s.clone()]),
        Value::Array(items) => items
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("{v} is not a string"))
            })
            .collect(),
        other => Err(format!("{other} is not a string or a list")),
    }
}

fn number(key: &str, value: &Value) -> Result<f64, String> {
    value
        .as_f64()
        .ok_or_else(|| format!("{key} is not a number"))
}

fn flag(key: &str, value: &Value) -> Result<bool, String> {
    value
        .as_bool()
        .ok_or_else(|| format!("{key} is not true or false"))
}

fn count(key: &str, value: &Value) -> Result<Count, String> {
    if let Some(amount) = value.as_u64() {
        return Ok(Count {
            amount: amount as usize,
            of: CountOf::ThisMob,
        });
    }
    let Value::Object(map) = value else {
        return Err(format!("{key} is not a number or an object"));
    };
    let mut amount = None;
    let mut of = CountOf::ThisMob;
    for (k, v) in map {
        match k.as_str() {
            "amount" => amount = v.as_u64(),
            "mob" => of = CountOf::Mobs(strings(v)?),
            "mod" => of = CountOf::Mods(strings(v)?),
            "hostile" if v.as_bool() == Some(true) => of = CountOf::Hostile,
            "passive" if v.as_bool() == Some(true) => of = CountOf::Passive,
            "all" if v.as_bool() == Some(true) => of = CountOf::All,
            other => return Err(format!("{key}.{other} is not supported yet")),
        }
    }
    Ok(Count {
        amount: amount.ok_or_else(|| format!("{key} has no amount"))? as usize,
        of,
    })
}

/// Read the shared conditions out of `rule`, leaving the keys it does not know in `rest`.
fn conditions(rule: &Map<String, Value>, rest: &mut Vec<String>) -> Result<Conditions, String> {
    let mut c = Conditions::default();
    for (key, value) in rule {
        match key.as_str() {
            "mob" => c.mobs = Some(strings(value)?),
            "mod" => c.mods = Some(strings(value)?),
            "hostile" => c.hostile = Some(flag(key, value)?),
            "passive" => c.passive = Some(flag(key, value)?),
            "dimension" => c.dimensions = Some(strings(value)?),
            "biome" => c.biomes = Some(strings(value)?),
            "biometags" => c.biome_tags = Some(strings(value)?),
            "minheight" => c.min_height = Some(number(key, value)?),
            "maxheight" => c.max_height = Some(number(key, value)?),
            "minlight" => c.min_light = Some(number(key, value)? as i32),
            "maxlight" => c.max_light = Some(number(key, value)? as i32),
            "mintime" => c.min_time = Some(number(key, value)? as i64),
            "maxtime" => c.max_time = Some(number(key, value)? as i64),
            "seesky" => c.see_sky = Some(flag(key, value)?),
            "weather" => c.weather = value.as_str().map(str::to_ascii_lowercase),
            "difficulty" => c.difficulty = value.as_str().map(str::to_ascii_lowercase),
            "random" => c.random = Some(number(key, value)?),
            "spawner" => c.spawner = Some(flag(key, value)?),
            "player" => c.player = Some(flag(key, value)?),
            "mincount" => c.min_count = Some(count(key, value)?),
            "maxcount" => c.max_count = Some(count(key, value)?),
            other => rest.push(other.to_owned()),
        }
    }
    Ok(c)
}

fn rules_of<T>(
    text: &str,
    mut one: impl FnMut(&Map<String, Value>, &mut Vec<String>) -> Result<T, String>,
) -> Result<Parsed<T>, String> {
    let json: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let Value::Array(items) = json else {
        return Err("the file is not a list of rules".to_owned());
    };
    let mut parsed = Parsed {
        rules: Vec::new(),
        skipped: Vec::new(),
    };
    for (index, item) in items.iter().enumerate() {
        let Value::Object(rule) = item else {
            parsed.skipped.push((index, "not an object".to_owned()));
            continue;
        };
        let mut rest = Vec::new();
        match one(rule, &mut rest) {
            Ok(_) if !rest.is_empty() => parsed
                .skipped
                .push((index, format!("not supported yet: {}", rest.join(", ")))),
            Ok(rule) => parsed.rules.push((index, rule)),
            Err(reason) => parsed.skipped.push((index, reason)),
        }
    }
    Ok(parsed)
}

/// Read `spawn.json`.
pub fn parse_spawn(text: &str) -> Result<Parsed<SpawnRule>, String> {
    rules_of(text, |rule, rest| {
        let when = match rule.get("when").and_then(Value::as_str) {
            None | Some("position") => When::Position,
            Some("finalize") => When::Finalize,
            Some("onjoin") => When::OnJoin,
            Some(other) => return Err(format!("when {other} is not supported yet")),
        };
        let outcome = match rule.get("result").and_then(Value::as_str) {
            Some("allow") => Outcome::Allow,
            Some("deny") => Outcome::Deny,
            None | Some("default") => Outcome::Default,
            Some(other) => return Err(format!("result {other} is not supported yet")),
        };
        let mut map = rule.clone();
        map.remove("when");
        map.remove("result");
        Ok(SpawnRule {
            when,
            conditions: conditions(&map, rest)?,
            outcome,
        })
    })
}

/// Read `experience.json`.
pub fn parse_experience(text: &str) -> Result<Parsed<XpRule>, String> {
    rules_of(text, |rule, rest| {
        let mut map = rule.clone();
        let deny = match map.remove("result").as_ref().and_then(Value::as_str) {
            Some("deny") => true,
            None | Some("allow") | Some("default") => false,
            Some(other) => return Err(format!("result {other} is not supported yet")),
        };
        let set = map
            .remove("setxp")
            .map(|v| number("setxp", &v))
            .transpose()?;
        let multiply = map
            .remove("multxp")
            .map(|v| number("multxp", &v))
            .transpose()?;
        let add = map
            .remove("addxp")
            .map(|v| number("addxp", &v))
            .transpose()?;
        Ok(XpRule {
            conditions: conditions(&map, rest)?,
            change: XpChange {
                deny,
                set: set.map(|v| v as i32),
                multiply,
                add: add.map(|v| v as i32),
            },
        })
    })
}

fn namespace(id: &str) -> &str {
    id.split_once(':').map_or("minecraft", |(ns, _)| ns)
}

fn listed(list: &Option<Vec<String>>, value: &str) -> bool {
    list.as_ref().is_none_or(|l| l.iter().any(|v| v == value))
}

impl Conditions {
    /// Whether every condition holds. `rule` is the rule's index, for its random draw.
    pub fn hold(&self, facts: &dyn Facts, rule: usize) -> bool {
        let kind = facts.kind();
        if !listed(&self.mobs, kind) || !listed(&self.mods, namespace(kind)) {
            return false;
        }
        if let Some(want) = self.hostile
            && facts.hostile() != Some(want)
        {
            return false;
        }
        if let Some(want) = self.passive
            && facts.hostile() != Some(!want)
        {
            return false;
        }
        if !listed(&self.dimensions, facts.dimension()) {
            return false;
        }
        let height = facts.height();
        if self.min_height.is_some_and(|m| height < m)
            || self.max_height.is_some_and(|m| height > m)
        {
            return false;
        }
        if let Some(spawner) = self.spawner
            && (facts.reason() == Reason::Spawner) != spawner
        {
            return false;
        }
        if let Some(player) = self.player
            && facts.by_player() != Some(player)
        {
            return false;
        }
        if self.biomes.is_some() && !facts.biome().is_some_and(|b| listed(&self.biomes, &b)) {
            return false;
        }
        if let Some(tags) = &self.biome_tags
            && !tags.iter().any(|t| facts.biome_in_tag(t))
        {
            return false;
        }
        if self.min_light.is_some() || self.max_light.is_some() {
            let Some(light) = facts.light() else {
                return false;
            };
            if self.min_light.is_some_and(|m| light < m)
                || self.max_light.is_some_and(|m| light > m)
            {
                return false;
            }
        }
        if self.min_time.is_some() || self.max_time.is_some() {
            let Some(time) = facts.day_time().map(|t| t.rem_euclid(24_000)) else {
                return false;
            };
            if self.min_time.is_some_and(|m| time < m) || self.max_time.is_some_and(|m| time > m) {
                return false;
            }
        }
        if let Some(want) = self.see_sky
            && facts.sees_sky() != Some(want)
        {
            return false;
        }
        if let Some(weather) = &self.weather {
            let now = match weather.as_str() {
                "rain" => facts.raining(),
                "thunder" => facts.thundering(),
                "clear" => !facts.raining(),
                _ => false,
            };
            if !now {
                return false;
            }
        }
        if let Some(want) = &self.difficulty
            && facts.difficulty().as_deref() != Some(want.as_str())
        {
            return false;
        }
        if let Some(count) = &self.min_count
            && facts.count(&count.of) < count.amount
        {
            return false;
        }
        if let Some(count) = &self.max_count
            && facts.count(&count.of) >= count.amount
        {
            return false;
        }
        if let Some(chance) = self.random
            && facts.random(rule) >= chance
        {
            return false;
        }
        true
    }
}

/// The outcome of the first spawn rule for `when` whose conditions hold.
pub fn spawn_outcome(rules: &[(usize, SpawnRule)], when: When, facts: &dyn Facts) -> Outcome {
    rules
        .iter()
        .filter(|(_, r)| r.when == when)
        .find(|(index, r)| r.conditions.hold(facts, *index))
        .map_or(Outcome::Default, |(_, r)| r.outcome)
}

/// The experience after the first experience rule whose conditions hold.
pub fn experience(rules: &[(usize, XpRule)], amount: i32, facts: &dyn Facts) -> i32 {
    rules
        .iter()
        .find(|(index, r)| r.conditions.hold(facts, *index))
        .map_or(amount, |(_, r)| r.change.apply(amount))
}

/// A deterministic draw in [0, 1) from the check's inputs (SplitMix64), so a replayed tick
/// draws the same.
pub fn draw(seed: [u64; 4]) -> f64 {
    let mut x = seed.iter().fold(0x9E37_79B9_7F4A_7C15u64, |acc, s| {
        acc.rotate_left(17) ^ s.wrapping_mul(0xBF58_476D_1CE4_E5B9)
    });
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 11) as f64 / (1u64 << 53) as f64
}
