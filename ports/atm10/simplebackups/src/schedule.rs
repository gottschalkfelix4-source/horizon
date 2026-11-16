//! When a backup runs, what kind it is, and which old backups go.
use crate::config::{BackupType, Config};

/// The mod's state, kept in the world (`horizon-mod-data/simplebackups/state.json`).
#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize, PartialEq)]
#[serde(default)]
pub struct State {
    /// Unix milliseconds of the last backup's start; 0 before the first.
    pub last_backup: u64,
    /// Unix milliseconds of the last full backup's start.
    pub last_full_backup: u64,
    /// `/backup pause` holds the timer until `/backup unpause`.
    pub paused: bool,
}

const MINUTE: u64 = 60_000;

/// Whether the timer asks for a backup now. `played` says whether a player was online since
/// the last backup.
pub fn due(config: &Config, state: &State, now: u64, played: bool) -> bool {
    config.enabled
        && !state.paused
        && (played || config.no_player_backups)
        && now.saturating_sub(state.last_backup) >= config.timer.max(1) * MINUTE
}

/// Files changed after this time go into the backup; 0 means every file (a full backup).
pub fn changed_since(config: &Config, state: &State, now: u64) -> u64 {
    let full_due = state.last_full_backup == 0
        || now.saturating_sub(state.last_full_backup) >= config.full_backup_timer * MINUTE;
    match config.backup_type {
        BackupType::FullBackups => 0,
        _ if full_due => 0,
        BackupType::ModifiedSinceLast => state.last_backup,
        BackupType::ModifiedSinceFull => state.last_full_backup,
    }
}

/// One backup on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Existing {
    pub name: String,
    pub modified: u64,
    pub size: u64,
}

/// The backups to delete, oldest first: past `keep` in number, then while the folder is larger
/// than `max_bytes`. The newest backup is never deleted.
pub fn to_delete(mut backups: Vec<Existing>, keep: usize, max_bytes: Option<u64>) -> Vec<String> {
    backups.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| b.name.cmp(&a.name))
    });
    let keep = keep.max(1);
    let mut doomed: Vec<Existing> = backups.split_off(keep.min(backups.len()));
    if let Some(max) = max_bytes {
        let mut total: u64 = backups.iter().map(|b| b.size).sum();
        while total > max && backups.len() > 1 {
            let oldest = backups.pop().expect("more than one");
            total -= oldest.size;
            doomed.push(oldest);
        }
    }
    doomed.sort_by_key(|b| b.modified);
    doomed.into_iter().map(|b| b.name).collect()
}

/// A backup's file name: the world's name and the start time, `YYYY-MM-DD_HH-MM-SS` in UTC.
pub fn file_name(world: &str, unix_millis: u64, full: bool) -> String {
    let secs = unix_millis / 1000;
    let (days, rest) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = civil_from_days(days as i64);
    let kind = if full { "" } else { "_partial" };
    format!(
        "{world}_{y:04}-{m:02}-{d:02}_{:02}-{:02}-{:02}{kind}.zip",
        rest / 3600,
        rest / 60 % 60,
        rest % 60
    )
}

/// Howard Hinnant's days-to-civil conversion.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}
