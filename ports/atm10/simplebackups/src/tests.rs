use super::*;
use config::{BackupType, parse_size};

const MIN: u64 = 60_000;

fn backup(name: &str, modified: u64, size: u64) -> Existing {
    Existing {
        name: name.into(),
        modified,
        size,
    }
}

#[test]
fn sizes_read_binary_units() {
    assert_eq!(parse_size("25 GB"), Some(25 << 30));
    assert_eq!(parse_size("500MB"), Some(500 << 20));
    assert_eq!(parse_size("1.5 kb"), Some(1536));
    assert_eq!(parse_size("12"), Some(12));
    assert_eq!(parse_size("lots"), None);
}

#[test]
fn the_timer_waits_its_minutes_and_for_a_player() {
    let config = Config::default(); // 120 minutes, no backups without players
    let state = State {
        last_backup: 1_000 * MIN,
        ..State::default()
    };
    assert!(!schedule::due(&config, &state, 1_119 * MIN, true));
    assert!(schedule::due(&config, &state, 1_120 * MIN, true));
    assert!(!schedule::due(&config, &state, 1_120 * MIN, false));
    let paused = State {
        paused: true,
        ..state.clone()
    };
    assert!(!schedule::due(&config, &paused, 2_000 * MIN, true));
    let always = Config {
        no_player_backups: true,
        ..config
    };
    assert!(schedule::due(&always, &state, 1_120 * MIN, false));
}

#[test]
fn partial_backups_fall_back_to_full_ones_on_their_timer() {
    let config = Config {
        backup_type: BackupType::ModifiedSinceFull,
        full_backup_timer: 60,
        ..Config::default()
    };
    let state = State {
        last_backup: 130 * MIN,
        last_full_backup: 100 * MIN,
        paused: false,
    };
    assert_eq!(
        schedule::changed_since(&config, &state, 150 * MIN),
        100 * MIN
    );
    let since_last = Config {
        backup_type: BackupType::ModifiedSinceLast,
        ..config.clone()
    };
    assert_eq!(
        schedule::changed_since(&since_last, &state, 150 * MIN),
        130 * MIN
    );
    assert_eq!(schedule::changed_since(&config, &state, 160 * MIN), 0);
    assert_eq!(schedule::changed_since(&config, &State::default(), 5), 0);
    assert_eq!(
        schedule::changed_since(&Config::default(), &state, 150 * MIN),
        0
    );
}

#[test]
fn old_backups_go_past_the_count_then_past_the_size() {
    let backups = vec![
        backup("a", 1, 10),
        backup("b", 2, 10),
        backup("c", 3, 10),
        backup("d", 4, 10),
    ];
    assert_eq!(schedule::to_delete(backups.clone(), 2, None), ["a", "b"]);
    assert_eq!(
        schedule::to_delete(backups.clone(), 10, Some(25)),
        ["a", "b"]
    );
    // The newest is kept even when it alone is over the budget.
    assert_eq!(schedule::to_delete(backups, 10, Some(1)), ["a", "b", "c"]);
}

#[test]
fn backup_names_carry_the_utc_time() {
    // 2024-02-29 13:05:09 UTC.
    assert_eq!(
        schedule::file_name("world", 1_709_211_909_000, true),
        "world_2024-02-29_13-05-09.zip"
    );
    assert_eq!(
        schedule::file_name("w", 0, false),
        "w_1970-01-01_00-00-00_partial.zip"
    );
}

#[test]
fn a_world_folder_zips_under_its_own_name_without_the_lock() {
    let root = std::env::temp_dir().join(format!("simplebackups-test-{}", std::process::id()));
    let world = root.join("MyWorld");
    std::fs::create_dir_all(world.join("region")).unwrap();
    std::fs::write(world.join("level.dat"), b"level").unwrap();
    std::fs::write(world.join("session.lock"), b"lock").unwrap();
    std::fs::write(world.join("region/r.0.0.mca"), vec![7u8; 5000]).unwrap();
    let target = root.join("out/backup.zip");
    archive::write(&world, &target, 0, Some(6)).unwrap();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&target).unwrap()).unwrap();
    let mut names: Vec<String> = zip.file_names().filter_map(Result::ok).map(|n| n.into_owned()).collect();
    names.sort();
    assert_eq!(names, ["MyWorld/level.dat", "MyWorld/region/r.0.0.mca"]);
    let mut region = Vec::new();
    std::io::Read::read_to_end(
        &mut zip.by_name("MyWorld/region/r.0.0.mca").unwrap(),
        &mut region,
    )
    .unwrap();
    assert_eq!(region, vec![7u8; 5000]);
    assert!(!target.with_extension("zip.part").exists());
    std::fs::remove_dir_all(root).unwrap();
}
