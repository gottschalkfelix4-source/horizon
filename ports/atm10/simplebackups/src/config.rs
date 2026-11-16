//! `config/simplebackups.json`. The original keeps a NeoForge TOML config with these settings;
//! the names here follow its descriptions and the defaults are assumed (README, "To check").
use serde::Deserialize;

/// What one backup holds.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BackupType {
    /// Every file of the world, each time.
    #[default]
    FullBackups,
    /// Only files changed since the last backup of any kind, with a full one at least every
    /// `full_backup_timer` minutes.
    ModifiedSinceLast,
    /// Only files changed since the last full backup, with the same full-backup rule.
    ModifiedSinceFull,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub backup_type: BackupType,
    /// Minutes between backups.
    pub timer: u64,
    /// Minutes after which a modified-only backup type makes a full one again.
    pub full_backup_timer: u64,
    /// Backups kept; older ones are deleted.
    pub backups_to_keep: usize,
    /// The most the backup folder may hold, as `25 GB`, `500 MB`; older backups go first.
    pub max_disk_size: String,
    /// Deflate level 0 to 9; -1 for the library's default.
    pub compression_level: i64,
    /// Tell the players when a backup starts and ends.
    pub send_messages: bool,
    /// Back up even when nobody played since the last backup.
    pub no_player_backups: bool,
    /// The folder backups go to, relative to the game folder.
    pub output_path: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            backup_type: BackupType::FullBackups,
            timer: 120,
            full_backup_timer: 1440,
            backups_to_keep: 10,
            max_disk_size: "25 GB".to_owned(),
            compression_level: -1,
            send_messages: true,
            no_player_backups: false,
            output_path: "simplebackups".to_owned(),
        }
    }
}

impl Config {
    pub fn parse(bytes: &[u8]) -> Self {
        serde_json::from_slice(bytes).unwrap_or_default()
    }

    /// `max_disk_size` in bytes; None for an unreadable value (no size limit then).
    pub fn max_disk_bytes(&self) -> Option<u64> {
        parse_size(&self.max_disk_size)
    }
}

/// `25 GB`, `500MB`, `1 TB`, `1024 KB` or plain bytes, in binary units.
pub fn parse_size(text: &str) -> Option<u64> {
    let text = text.trim();
    let split = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let number: f64 = number.parse().ok()?;
    let scale: u64 = match unit.trim().to_ascii_uppercase().as_str() {
        "" | "B" => 1,
        "KB" => 1 << 10,
        "MB" => 1 << 20,
        "GB" => 1 << 30,
        "TB" => 1 << 40,
        _ => return None,
    };
    Some((number * scale as f64) as u64)
}
