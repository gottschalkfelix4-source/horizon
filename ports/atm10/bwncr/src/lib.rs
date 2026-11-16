//! Bad Wither No Cookie - Reloaded for Horizon: the loud sounds every player hears (the wither's,
//! the ender dragon's, thunder, the wandering trader's) are dropped before they go out, with
//! explosions and any listed sound on request.
//!
//! Written from the mod's public description (its options, defaults and commands), never from
//! its code (docs/modpacks.md, "Licences"). What the port does, and what a harness against the
//! NeoForge original still has to confirm, is in README.md.
//!
//! How it works: `play_level_sound_at_position` and `play_level_sound_at_entity` see every sound
//! a level plays; a sound in an enabled group, or in the `silenceUs` list, is cancelled, so no
//! player hears it. The options live in `config/bwncr-common.toml` and `/bwncr` changes them
//! and writes the file back.
use std::sync::RwLock;

use mod_api::{
    Command, CommandArgumentValue, CommandSource, Config, ConfigKind, ConfigLoadingEvent,
    ConfigPath, ConfigReloadingEvent, Flow, Hooks, HostSide, PlayLevelSoundAtEntityEvent,
    PlayLevelSoundAtPositionEvent, RegisterCommandsEvent, RegisterConfigsEvent, RunModCommandEvent,
    ServerHost, Text,
};

const MOD_ID: &str = "bwncr";
const CONFIG_FILE: &str = "bwncr-common.toml";

/// A group of sounds one option silences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Wither,
    Dragon,
    Lightning,
    Trader,
    Explosion,
}

impl Group {
    pub const ALL: [Group; 5] = [
        Group::Wither,
        Group::Dragon,
        Group::Lightning,
        Group::Trader,
        Group::Explosion,
    ];

    /// The option's name in the config file and in `/bwncr`.
    pub fn option(self) -> &'static str {
        match self {
            Group::Wither => "silenceWither",
            Group::Dragon => "silenceDragon",
            Group::Lightning => "silenceLightning",
            Group::Trader => "silenceTrader",
            Group::Explosion => "silenceExplosion",
        }
    }

    /// The vanilla sound events the group covers (path in `minecraft`).
    fn sounds(self) -> &'static [&'static str] {
        match self {
            Group::Wither => &["entity.wither.spawn", "entity.wither.death"],
            Group::Dragon => &[
                "entity.ender_dragon.death",
                "entity.ender_dragon.growl",
                "entity.ender_dragon.flap",
            ],
            Group::Lightning => &["entity.lightning_bolt.thunder"],
            Group::Trader => &["entity.wandering_trader.ambient"],
            Group::Explosion => &[
                "entity.generic.explode",
                "entity.tnt.primed",
                "entity.creeper.primed",
                "entity.wind_charge.wind_burst",
            ],
        }
    }

    fn of_option(name: &str) -> Option<Group> {
        Group::ALL
            .into_iter()
            .find(|g| g.option().eq_ignore_ascii_case(name))
    }
}

/// The mod's options, as `bwncr-common.toml` holds them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    /// One flag per [`Group`], in [`Group::ALL`]'s order.
    pub groups: [bool; 5],
    /// Log each silenced sound.
    pub debug: bool,
    /// Extra sounds to silence (`silenceUs`), as full ids.
    pub extra: Vec<String>,
}

impl Settings {
    pub const DEFAULT: Settings = Settings {
        groups: [true, true, true, true, false],
        debug: false,
        extra: Vec::new(),
    };

    fn group(&self, g: Group) -> bool {
        self.groups[g as usize]
    }

    fn set_group(&mut self, g: Group, on: bool) {
        self.groups[g as usize] = on;
    }

    /// Whether a sound with id `sound` is dropped.
    pub fn silences(&self, sound: &str) -> bool {
        let sound = canonical(sound);
        let vanilla = sound.strip_prefix("minecraft:");
        Group::ALL
            .into_iter()
            .any(|g| self.group(g) && vanilla.is_some_and(|path| g.sounds().contains(&path)))
            || self.extra.contains(&sound)
    }

    /// Reads the options from the config's text. Unknown keys and malformed lines are skipped,
    /// so a hand-edited file keeps what it can; a missing key keeps its default.
    pub fn parse(text: &str) -> Settings {
        let mut out = Settings::DEFAULT;
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            if key == "silenceUs" {
                out.extra = parse_list(value);
                continue;
            }
            let Some(on) = parse_bool(value) else {
                continue;
            };
            if key == "debugMode" {
                out.debug = on;
            } else if let Some(g) = Group::of_option(key) {
                out.set_group(g, on);
            }
        }
        out
    }

    /// Writes the options back as the config's text.
    pub fn render(&self) -> String {
        let mut out = String::from("[general]\n");
        for g in Group::ALL {
            out.push_str(&format!("{} = {}\n", g.option(), self.group(g)));
        }
        out.push_str(&format!("debugMode = {}\n", self.debug));
        let list: Vec<String> = self.extra.iter().map(|s| format!("\"{s}\"")).collect();
        out.push_str(&format!("silenceUs = [{}]\n", list.join(", ")));
        out
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn parse_list(value: &str) -> Vec<String> {
    let inner = value.trim().trim_start_matches('[').trim_end_matches(']');
    inner
        .split(',')
        .map(|s| s.trim().trim_matches('"').trim_matches('\''))
        .filter(|s| !s.is_empty())
        .map(canonical)
        .collect()
}

/// `minecraft:` added to an id without a namespace.
fn canonical(id: &str) -> String {
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}

static SETTINGS: RwLock<Settings> = RwLock::new(Settings::DEFAULT);

fn settings() -> Settings {
    SETTINGS
        .read()
        .map(|s| s.clone())
        .unwrap_or(Settings::DEFAULT)
}

fn store(s: Settings) {
    if let Ok(mut slot) = SETTINGS.write() {
        *slot = s;
    }
}

/// Drops `sound` when the options say so; `cancelled` is the event's flag.
fn filter(sound: &str, cancelled: &mut bool) {
    let s = settings();
    if s.silences(sound) {
        *cancelled = true;
        if s.debug {
            log::info!("[bwncr] silenced {sound}");
        }
    }
}

/// What one `/bwncr` invocation asks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    Show,
    Set(String, bool),
    Toggle(String),
    ExtraAdd(String),
    ExtraRemove(String),
    ExtraClear,
}

/// The answer to show: a translation key and its arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    pub key: &'static str,
    pub args: Vec<String>,
}

impl Reply {
    fn new(key: &'static str, args: Vec<String>) -> Self {
        Reply { key, args }
    }
}

/// Applies `request` to `s`: the replies to show, and whether `s` changed.
pub fn apply(s: &mut Settings, request: &Request) -> (Vec<Reply>, bool) {
    match request {
        Request::Show => {
            let mut lines: Vec<Reply> = Group::ALL
                .into_iter()
                .map(|g| {
                    Reply::new(
                        "commands.bwncr.value",
                        vec![g.option().into(), s.group(g).to_string()],
                    )
                })
                .collect();
            lines.push(Reply::new(
                "commands.bwncr.value",
                vec!["debugMode".into(), s.debug.to_string()],
            ));
            lines.push(Reply::new(
                "commands.bwncr.value",
                vec!["silenceUs".into(), s.extra.join(", ")],
            ));
            (lines, false)
        }
        Request::Set(option, on) if option.eq_ignore_ascii_case("all") => {
            for g in Group::ALL {
                s.set_group(g, *on);
            }
            (
                vec![Reply::new("commands.bwncr.all", vec![on.to_string()])],
                true,
            )
        }
        Request::Set(option, on) => set_option(s, option, |_| *on),
        Request::Toggle(option) => set_option(s, option, |old| !old),
        Request::ExtraAdd(sound) => {
            let sound = canonical(sound);
            let added = !s.extra.contains(&sound);
            if added {
                s.extra.push(sound.clone());
            }
            (
                vec![Reply::new("commands.bwncr.extra.added", vec![sound])],
                added,
            )
        }
        Request::ExtraRemove(sound) => {
            let sound = canonical(sound);
            let before = s.extra.len();
            s.extra.retain(|x| *x != sound);
            let removed = s.extra.len() != before;
            let key = if removed {
                "commands.bwncr.extra.removed"
            } else {
                "commands.bwncr.extra.missing"
            };
            (vec![Reply::new(key, vec![sound])], removed)
        }
        Request::ExtraClear => {
            let changed = !s.extra.is_empty();
            s.extra.clear();
            (
                vec![Reply::new("commands.bwncr.extra.cleared", vec![])],
                changed,
            )
        }
    }
}

fn set_option(s: &mut Settings, option: &str, value: impl Fn(bool) -> bool) -> (Vec<Reply>, bool) {
    let (name, old) = if option.eq_ignore_ascii_case("debugMode") {
        ("debugMode", s.debug)
    } else if let Some(g) = Group::of_option(option) {
        (g.option(), s.group(g))
    } else {
        return (
            vec![Reply::new("commands.bwncr.unknown", vec![option.into()])],
            false,
        );
    };
    let new = value(old);
    if name == "debugMode" {
        s.debug = new;
    } else if let Some(g) = Group::of_option(name) {
        s.set_group(g, new);
    }
    (
        vec![Reply::new(
            "commands.bwncr.value",
            vec![name.into(), new.to_string()],
        )],
        new != old,
    )
}

fn request_of(event: &RunModCommandEvent<'_>) -> Option<Request> {
    let lits: Vec<&str> = event.command.literals.iter().map(String::as_str).collect();
    let arg = |name: &str| {
        event
            .command
            .arguments
            .iter()
            .find(|a| a.name == name)
            .map(|a| &a.value)
    };
    let text = |name: &str| match arg(name)? {
        CommandArgumentValue::String(s) => Some(s.clone()),
        CommandArgumentValue::Id(id) => Some(id.to_string()),
        _ => None,
    };
    match lits.as_slice() {
        ["bwncr", "show"] => Some(Request::Show),
        ["bwncr", "set"] => match arg("value")? {
            CommandArgumentValue::Boolean(on) => Some(Request::Set(text("option")?, *on)),
            _ => None,
        },
        ["bwncr", "toggle"] => Some(Request::Toggle(text("option")?)),
        ["bwncr", "silenceus", "add"] => Some(Request::ExtraAdd(text("sound")?)),
        ["bwncr", "silenceus", "remove"] => Some(Request::ExtraRemove(text("sound")?)),
        ["bwncr", "silenceus", "clear"] => Some(Request::ExtraClear),
        _ => None,
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register_configs(&self, _: HostSide, event: &mut RegisterConfigsEvent<'_>) {
        event.configs.push(Config::new(
            MOD_ID,
            ConfigKind::Common,
            &Settings::DEFAULT.render(),
        ));
    }

    fn config_loading(&self, _: HostSide, event: &mut ConfigLoadingEvent<'_>) {
        if event.config.mod_id == MOD_ID {
            store(Settings::parse(&event.config.text));
        }
    }

    fn config_reloading(&self, _: HostSide, event: &mut ConfigReloadingEvent<'_>) {
        if event.config.mod_id == MOD_ID {
            store(Settings::parse(&event.config.text));
        }
    }

    fn play_level_sound_at_position(
        &self,
        _: HostSide,
        event: &mut PlayLevelSoundAtPositionEvent<'_>,
    ) -> Flow {
        filter(&event.sound.to_string(), &mut event.cancelled);
        Flow::Continue
    }

    fn play_level_sound_at_entity(
        &self,
        _: HostSide,
        event: &mut PlayLevelSoundAtEntityEvent<'_>,
    ) -> Flow {
        filter(&event.sound.to_string(), &mut event.cancelled);
        Flow::Continue
    }

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.extend([
            Command::op("bwncr show", 2),
            Command::op("bwncr set <option:word> <value:boolean>", 2),
            Command::op("bwncr toggle <option:word>", 2),
            Command::op("bwncr silenceus add <sound:id>", 2),
            Command::op("bwncr silenceus remove <sound:id>", 2),
            Command::op("bwncr silenceus clear", 2),
        ]);
    }

    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        let Some(request) = request_of(event) else {
            return Flow::Continue;
        };
        let mut s = settings();
        let (replies, changed) = apply(&mut s, &request);
        if changed {
            let text = s.render();
            store(s);
            if let Ok(path) = ConfigPath::new(CONFIG_FILE)
                && let Err(e) = host.config_files().write(&path, text.as_bytes())
            {
                log::warn!("[bwncr] could not save {CONFIG_FILE}: {e:?}");
            }
        }
        for reply in replies {
            match event.source {
                CommandSource::Player { player, .. } => {
                    let args = reply.args.iter().cloned().map(Text::literal).collect();
                    let _ = player.tell(host, Text::translate(reply.key, args));
                }
                _ => log::info!("[bwncr] {} {}", reply.key, reply.args.join(" ")),
            }
        }
        Flow::Cancel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_silence_the_loud_groups_but_not_explosions() {
        let s = Settings::DEFAULT;
        assert!(s.silences("minecraft:entity.wither.spawn"));
        assert!(s.silences("entity.ender_dragon.death"));
        assert!(s.silences("minecraft:entity.lightning_bolt.thunder"));
        assert!(s.silences("minecraft:entity.wandering_trader.ambient"));
        assert!(!s.silences("minecraft:entity.generic.explode"));
        assert!(!s.silences("minecraft:entity.zombie.ambient"));
        assert!(!s.silences("othermod:entity.wither.spawn"));
    }

    #[test]
    fn the_config_text_round_trips_and_ignores_junk() {
        let text = "# comment\n[general]\nsilenceWither = false\nsilenceExplosion = true # bangs\n\
                    debugMode = true\nsilenceUs = [\"entity.cat.ambient\", 'mymod:boom']\nnonsense\nsilenceDragon = maybe\n";
        let s = Settings::parse(text);
        assert_eq!(s.groups, [false, true, true, true, true]);
        assert!(s.debug);
        assert_eq!(
            s.extra,
            vec![
                "minecraft:entity.cat.ambient".to_owned(),
                "mymod:boom".to_owned()
            ]
        );
        assert!(s.silences("minecraft:entity.cat.ambient"));
        assert!(!s.silences("minecraft:entity.wither.death"));
        assert_eq!(Settings::parse(&s.render()), s);
    }

    #[test]
    fn commands_change_options_and_report_whether_anything_changed() {
        let mut s = Settings::DEFAULT;
        let (_, changed) = apply(&mut s, &Request::Toggle("silencewither".into()));
        assert!(changed);
        assert!(!s.silences("minecraft:entity.wither.death"));
        let (_, changed) = apply(&mut s, &Request::Set("all".into(), true));
        assert!(changed);
        assert!(s.silences("minecraft:entity.tnt.primed"));
        let (replies, changed) = apply(&mut s, &Request::Set("nope".into(), true));
        assert!(!changed);
        assert_eq!(replies[0].key, "commands.bwncr.unknown");
        let (_, changed) = apply(&mut s, &Request::ExtraAdd("entity.cow.ambient".into()));
        assert!(changed);
        let (_, changed) = apply(
            &mut s,
            &Request::ExtraAdd("minecraft:entity.cow.ambient".into()),
        );
        assert!(!changed);
        let (_, changed) = apply(&mut s, &Request::ExtraRemove("entity.cow.ambient".into()));
        assert!(changed);
        assert!(s.extra.is_empty());
    }
}
