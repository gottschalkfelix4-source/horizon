//! SFML, the Factory Manager's language: the slice this port reads and runs.
//!
//! ```text
//! NAME "smelting"
//! EVERY 20 TICKS DO
//!     INPUT 64 iron_ore, gold_ore FROM ores
//!     OUTPUT TO furnace TOP SIDE
//! END
//! ```
//!
//! Written from SFM's public examples, never from its grammar file: keywords are case-insensitive,
//! a statement's items come before `FROM`/`TO`, an item may carry its own limit, `*` matches any
//! run of characters, and an item without a namespace matches that path in any namespace.

/// The fewest ticks between two runs of a trigger without timer upgrades (SFM refuses less).
pub const MIN_INTERVAL: u32 = 20;

/// A face of an inventory a statement reaches it through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
    North,
    South,
    East,
    West,
}

/// One item entry of a statement: how many (none: no limit) of what.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub limit: Option<u64>,
    pub pattern: String,
}

impl Entry {
    /// Whether the item `id` (`namespace:path`) is one this entry names.
    pub fn matches(&self, id: &str) -> bool {
        let pattern = self.pattern.as_str();
        if pattern.contains(':') {
            return glob(pattern, id);
        }
        let path = id.split_once(':').map_or(id, |(_, path)| path);
        glob(pattern, path)
    }
}

/// `*` matches any run of characters; everything else matches itself.
fn glob(pattern: &str, text: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pattern.chars().collect(), text.chars().collect());
    let (mut pi, mut ti, mut star, mut mark) = (0, 0, None, 0);
    while ti < t.len() {
        if pi < p.len() && p[pi] != '*' && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

/// `INPUT ... FROM labels` or `OUTPUT ... TO labels`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Io {
    pub output: bool,
    pub entries: Vec<Entry>,
    pub labels: Vec<String>,
    pub side: Option<Side>,
}

/// `EVERY n TICKS DO ... END`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trigger {
    pub interval: u32,
    pub statements: Vec<Io>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Program {
    pub name: Option<String>,
    pub triggers: Vec<Trigger>,
}

/// Why a program was refused: a translation key and the word (counted from 1) it stopped at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub key: &'static str,
    pub word: usize,
}

fn tokens(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = source.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == ',' {
            chars.next();
            out.push(",".into());
        } else if c == '"' {
            chars.next();
            let text: String = chars.by_ref().take_while(|c| *c != '"').collect();
            out.push(format!("\"{text}"));
        } else {
            let mut word = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_whitespace() || c == ',' || c == '"' {
                    break;
                }
                word.push(c);
                chars.next();
            }
            out.push(word);
        }
    }
    out
}

struct Parser {
    words: Vec<String>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&str> {
        self.words.get(self.at).map(String::as_str)
    }
    fn is(&self, keyword: &str) -> bool {
        self.peek().is_some_and(|w| w.eq_ignore_ascii_case(keyword))
    }
    fn err(&self, key: &'static str) -> ParseError {
        ParseError {
            key,
            word: self.at + 1,
        }
    }
    fn expect(&mut self, keyword: &str, key: &'static str) -> Result<(), ParseError> {
        if !self.is(keyword) {
            return Err(self.err(key));
        }
        self.at += 1;
        Ok(())
    }
    fn number(&mut self) -> Option<u64> {
        let n = self.peek()?.parse().ok()?;
        self.at += 1;
        Some(n)
    }
    fn name(&mut self) -> Result<String, ParseError> {
        match self.peek() {
            Some(w) if is_name(w) => {
                let w = w.to_owned();
                self.at += 1;
                Ok(w)
            }
            _ => Err(self.err("sfm.horizon.error.expected_name")),
        }
    }
}

const KEYWORDS: [&str; 10] = [
    "every", "do", "end", "input", "output", "from", "to", "side", "name", ",",
];

fn is_name(word: &str) -> bool {
    !word.starts_with('"')
        && !KEYWORDS.iter().any(|k| word.eq_ignore_ascii_case(k))
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_:*./-".contains(c))
}

fn side(word: &str) -> Option<Side> {
    Some(match word.to_ascii_lowercase().as_str() {
        "top" => Side::Top,
        "bottom" => Side::Bottom,
        "north" => Side::North,
        "south" => Side::South,
        "east" => Side::East,
        "west" => Side::West,
        _ => return None,
    })
}

/// Reads a whole program; the first mistake refuses it.
pub fn parse(source: &str) -> Result<Program, ParseError> {
    let mut p = Parser {
        words: tokens(source),
        at: 0,
    };
    let mut program = Program::default();
    if p.is("name") {
        p.at += 1;
        match p.peek() {
            Some(w) if w.starts_with('"') => program.name = Some(w[1..].to_owned()),
            _ => return Err(p.err("sfm.horizon.error.expected_string")),
        }
        p.at += 1;
    }
    while p.peek().is_some() {
        program.triggers.push(trigger(&mut p)?);
    }
    Ok(program)
}

fn trigger(p: &mut Parser) -> Result<Trigger, ParseError> {
    p.expect("every", "sfm.horizon.error.expected_every")?;
    let at = p.at;
    let count = p
        .number()
        .ok_or_else(|| p.err("sfm.horizon.error.expected_number"))?;
    let ticks = match p.peek().map(str::to_ascii_lowercase).as_deref() {
        Some("tick" | "ticks") => count,
        Some("second" | "seconds") => count.saturating_mul(20),
        _ => return Err(p.err("sfm.horizon.error.expected_unit")),
    };
    p.at += 1;
    if ticks < u64::from(MIN_INTERVAL) {
        return Err(ParseError {
            key: "sfm.horizon.error.interval_too_short",
            word: at + 1,
        });
    }
    p.expect("do", "sfm.horizon.error.expected_do")?;
    let mut statements = Vec::new();
    while !p.is("end") {
        if p.peek().is_none() {
            return Err(p.err("sfm.horizon.error.expected_end"));
        }
        statements.push(io(p)?);
    }
    p.at += 1;
    Ok(Trigger {
        interval: u32::try_from(ticks).unwrap_or(u32::MAX),
        statements,
    })
}

fn io(p: &mut Parser) -> Result<Io, ParseError> {
    let output = if p.is("input") {
        false
    } else if p.is("output") {
        true
    } else {
        return Err(p.err("sfm.horizon.error.expected_statement"));
    };
    p.at += 1;
    let mut entries = Vec::new();
    let joiner = if output { "to" } else { "from" };
    while !p.is(joiner) {
        let limit = p.number();
        let pattern = if p.is(joiner) || p.is(",") {
            "*".to_owned()
        } else {
            p.name()?
        };
        entries.push(Entry { limit, pattern });
        if p.is(",") {
            p.at += 1;
        } else if !p.is(joiner) {
            return Err(p.err(if output {
                "sfm.horizon.error.expected_to"
            } else {
                "sfm.horizon.error.expected_from"
            }));
        }
    }
    p.at += 1;
    if entries.is_empty() {
        entries.push(Entry {
            limit: None,
            pattern: "*".into(),
        });
    }
    let mut labels = vec![p.name()?];
    while p.is(",") {
        p.at += 1;
        labels.push(p.name()?);
    }
    let mut face = None;
    if let Some(s) = p.peek().and_then(side) {
        p.at += 1;
        p.expect("side", "sfm.horizon.error.expected_side")?;
        face = Some(s);
    }
    Ok(Io {
        output,
        entries,
        labels,
        side: face,
    })
}

/// Where one statement reaches an inventory: its position and face.
pub type Endpoint = ([i32; 3], Option<Side>);

/// Runs one trigger: every `OUTPUT` takes from every `INPUT` above it in the same trigger, in
/// order, each item entry keeping its limit across the run. `resolve` gives a label's
/// inventories (only those on the manager's network); `mover` moves up to `max` items that the
/// filter accepts from one endpoint to another and answers how many went. Answers the total moved.
pub fn run(
    trigger: &Trigger,
    resolve: &dyn Fn(&str) -> Vec<[i32; 3]>,
    mover: &mut dyn FnMut(Endpoint, Endpoint, &dyn Fn(&str) -> bool, u64) -> u64,
) -> u64 {
    let mut input_left: Vec<Vec<Option<u64>>> = Vec::new();
    let mut moved_total = 0;
    for (index, statement) in trigger.statements.iter().enumerate() {
        input_left.push(statement.entries.iter().map(|e| e.limit).collect());
        if !statement.output {
            continue;
        }
        let mut output_left: Vec<Option<u64>> = statement.entries.iter().map(|e| e.limit).collect();
        let targets: Vec<[i32; 3]> = statement.labels.iter().flat_map(|l| resolve(l)).collect();
        for (input_index, input) in trigger.statements[..index].iter().enumerate() {
            if input.output {
                continue;
            }
            let sources: Vec<[i32; 3]> = input.labels.iter().flat_map(|l| resolve(l)).collect();
            for (ie, in_entry) in input.entries.iter().enumerate() {
                for (oe, out_entry) in statement.entries.iter().enumerate() {
                    for &from in &sources {
                        for &to in &targets {
                            let budget = match (input_left[input_index][ie], output_left[oe]) {
                                (Some(a), Some(b)) => a.min(b),
                                (Some(a), None) | (None, Some(a)) => a,
                                (None, None) => u64::MAX,
                            };
                            if budget == 0 || from == to {
                                continue;
                            }
                            let filter = |id: &str| in_entry.matches(id) && out_entry.matches(id);
                            let went =
                                mover((from, input.side), (to, statement.side), &filter, budget);
                            if let Some(left) = &mut input_left[input_index][ie] {
                                *left -= went.min(*left);
                            }
                            if let Some(left) = &mut output_left[oe] {
                                *left -= went.min(*left);
                            }
                            moved_total += went;
                        }
                    }
                }
            }
        }
    }
    moved_total
}
