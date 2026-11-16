//! Simple Backups for Horizon: zips the world on a timer, keeps a set number of backups within
//! a disk budget, and lets operators start, pause and resume backups by command.
//!
//! Written from Simple Backups' public description and observed behaviour, never from its code
//! (docs/modpacks.md, "Licences"). See README.md for what is assumed.
//!
//! A backup: the server saves and stops writing (`save-all flush`, `save-off`, as an operator
//! would by hand), a worker thread zips the world folder into `<game>/simplebackups/`, and when
//! it is done the server writes again (`save-on`) and old backups beyond the limits are deleted.
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use mod_api::{
    Command, CommandArgumentValue, ConfigPath, Flow, Hooks, RegisterCommandsEvent,
    RunModCommandEvent, ServerHost, ServerStartedEvent, ServerStoppingEvent, ServerTickEvent,
    ServerTickPhase, Text,
};

pub mod archive;
pub mod config;
pub mod schedule;
mod text;

use config::Config;
use schedule::{Existing, State};
use text::{lit, tr};

/// How often, in ticks, the timer and a running backup are looked at.
const CHECK_EVERY: u64 = 20;

struct Running {
    worker: JoinHandle<std::io::Result<u64>>,
    started: Instant,
    quiet: bool,
}

static CONFIG: Mutex<Option<Config>> = Mutex::new(None);
static STATE: Mutex<Option<State>> = Mutex::new(None);
static RUNNING: Mutex<Option<Running>> = Mutex::new(None);
static PLAYED: AtomicBool = AtomicBool::new(false);
static TICKS: AtomicU64 = AtomicU64::new(0);

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn config() -> Config {
    CONFIG
        .lock()
        .ok()
        .and_then(|c| c.clone())
        .unwrap_or_default()
}

fn state() -> State {
    STATE
        .lock()
        .ok()
        .and_then(|s| s.clone())
        .unwrap_or_default()
}

fn state_path() -> Option<ConfigPath> {
    ConfigPath::new("state.json").ok()
}

fn store_state(host: &dyn ServerHost, new: State) {
    if let (Some(path), Ok(bytes)) = (state_path(), serde_json::to_vec(&new)) {
        let _ = host.world_data_files().write(&path, &bytes);
    }
    if let Ok(mut slot) = STATE.lock() {
        *slot = Some(new);
    }
}

fn tell(host: &dyn ServerHost, line: Text, quiet: bool) {
    if !quiet && config().send_messages {
        let _ = host.messages().broadcast(line);
    }
}

fn output_dir(host: &dyn ServerHost, config: &Config) -> PathBuf {
    let path = Path::new(&config.output_path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        host.instance_dir().join(path)
    }
}

fn anyone_online(host: &dyn ServerHost) -> bool {
    host.levels()
        .list_levels()
        .into_iter()
        .any(|level| level.online_players(host).is_some_and(|p| !p.is_empty()))
}

/// Start a backup now, unless one runs. False when none started.
fn start(host: &mut dyn ServerHost, quiet: bool) -> bool {
    let config = config();
    let Some(world) = host.world_dir() else {
        return false;
    };
    let Ok(mut running) = RUNNING.lock() else {
        return false;
    };
    if running.is_some() {
        return false;
    }
    let now = now_millis();
    let mut state = state();
    let changed_since = schedule::changed_since(&config, &state, now);
    let full = changed_since == 0;
    let name = world
        .file_name()
        .map_or_else(|| "world".into(), |n| n.to_string_lossy().into_owned());
    let target = output_dir(host, &config).join(schedule::file_name(&name, now, full));
    let commands = host.commands();
    commands.run_command("save-all flush");
    commands.run_command("save-off");
    tell(host, tr("simplebackups.backup_started", vec![]), quiet);
    let level = Some(config.compression_level);
    let worker = std::thread::Builder::new()
        .name("simplebackups".into())
        .spawn(move || archive::write(&world, &target, changed_since, level));
    let Ok(worker) = worker else {
        host.commands().run_command("save-on");
        return false;
    };
    *running = Some(Running {
        worker,
        started: Instant::now(),
        quiet,
    });
    drop(running);
    state.last_backup = now;
    if full {
        state.last_full_backup = now;
    }
    store_state(host, state);
    PLAYED.store(anyone_online(host), Ordering::Relaxed);
    true
}

/// Finish a backup whose worker is done (or wait for it with `wait`).
fn finish(host: &mut dyn ServerHost, wait: bool) {
    let done = {
        let Ok(mut running) = RUNNING.lock() else {
            return;
        };
        match running.as_ref() {
            Some(r) if wait || r.worker.is_finished() => running.take(),
            _ => None,
        }
    };
    let Some(done) = done else {
        return;
    };
    let result = done.worker.join();
    host.commands().run_command("save-on");
    let seconds = done.started.elapsed().as_secs();
    let line = match result {
        Ok(Ok(bytes)) => tr(
            "simplebackups.backup_finished",
            vec![lit(seconds), lit(bytes / (1 << 20))],
        ),
        Ok(Err(error)) => tr("simplebackups.backup_failed", vec![lit(error)]),
        Err(_) => tr("simplebackups.backup_failed", vec![lit("panic")]),
    };
    tell(host, line, done.quiet);
    prune(host);
}

/// Delete the backups past the count and size limits.
fn prune(host: &dyn ServerHost) {
    let config = config();
    let dir = output_dir(host, &config);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let existing = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(".zip"))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
            Some(Existing {
                name: e.file_name().to_string_lossy().into_owned(),
                modified: modified.as_millis() as u64,
                size: meta.len(),
            })
        })
        .collect();
    for name in schedule::to_delete(existing, config.backups_to_keep, config.max_disk_bytes()) {
        let _ = std::fs::remove_file(dir.join(name));
    }
}

fn set_paused(host: &dyn ServerHost, paused: bool) {
    let mut state = state();
    state.paused = paused;
    store_state(host, state);
}

fn feedback(host: &mut dyn ServerHost, key: &str) {
    let _ = host.commands().feedback(tr(key, vec![]));
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn server_started(&self, host: &mut dyn ServerHost, _: &mut ServerStartedEvent) {
        let config = ConfigPath::new("simplebackups.json")
            .ok()
            .and_then(|path| host.config_files().read(&path).ok())
            .map(|bytes| Config::parse(&bytes))
            .unwrap_or_default();
        let state: State = state_path()
            .and_then(|path| host.world_data_files().read(&path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        if let Ok(mut slot) = CONFIG.lock() {
            *slot = Some(config);
        }
        if let Ok(mut slot) = STATE.lock() {
            *slot = Some(state);
        }
        PLAYED.store(false, Ordering::Relaxed);
        TICKS.store(0, Ordering::Relaxed);
    }

    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        if !matches!(event.phase, ServerTickPhase::ServerTickEnd { .. }) {
            return;
        }
        let tick = TICKS.fetch_add(1, Ordering::Relaxed) + 1;
        if !tick.is_multiple_of(CHECK_EVERY) {
            return;
        }
        finish(host, false);
        if anyone_online(host) {
            PLAYED.store(true, Ordering::Relaxed);
        }
        if schedule::due(
            &config(),
            &state(),
            now_millis(),
            PLAYED.load(Ordering::Relaxed),
        ) {
            start(host, false);
        }
    }

    /// A backup still writing when the server stops is waited for, so it is complete.
    fn server_stopping(&self, host: &mut dyn ServerHost, _: &mut ServerStoppingEvent) {
        finish(host, true);
    }

    fn register_commands(&self, _: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.extend(
            [
                "backup start [quiet:boolean]",
                "backup pause",
                "backup unpause",
            ]
            .map(|usage| Command::op(usage, 2)),
        );
    }

    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        let path: Vec<&str> = event.command.literals.iter().map(String::as_str).collect();
        match path.as_slice() {
            ["backup", "start"] => {
                let quiet = matches!(
                    event.command.argument("quiet"),
                    Some(CommandArgumentValue::Boolean(true))
                );
                if !config().enabled {
                    feedback(host, "simplebackups.disabled");
                } else if !start(host, quiet) {
                    feedback(host, "simplebackups.already_running");
                }
            }
            ["backup", "pause"] => {
                set_paused(host, true);
                feedback(host, "simplebackups.paused");
            }
            ["backup", "unpause"] => {
                set_paused(host, false);
                feedback(host, "simplebackups.unpaused");
            }
            _ => return Flow::Continue,
        }
        Flow::Cancel
    }
}

#[cfg(test)]
mod tests;
