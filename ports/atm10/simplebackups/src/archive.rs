//! Writing a world folder into a zip.
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use zip::write::SimpleFileOptions;

/// Files never copied: the lock the running server holds.
const SKIPPED: &[&str] = &["session.lock"];

/// Every file under `root`, relative and `/`-separated, sorted.
pub fn files(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file()
                && !SKIPPED.contains(&entry.file_name().to_string_lossy().as_ref())
            {
                out.push(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
            }
        }
    }
    out.sort();
    Ok(out)
}

fn modified_millis(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(u64::MAX, |d| d.as_millis() as u64)
}

/// Zip the files of `world` changed after `changed_since` (0: all) into `target`, under a
/// top folder named after the world, as a world folder unzips. Written to a `.part` file first
/// and renamed, so a half-written backup is never mistaken for one. Returns the zip's size.
pub fn write(
    world: &Path,
    target: &Path,
    changed_since: u64,
    level: Option<i64>,
) -> io::Result<u64> {
    let top = world
        .file_name()
        .map_or_else(|| "world".into(), |n| n.to_string_lossy().into_owned());
    let partial = target.with_extension("zip.part");
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut zip = zip::ZipWriter::new(File::create(&partial)?);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(level.filter(|l| (0..=9).contains(l)));
    for relative in files(world)? {
        let path = world.join(&relative);
        if changed_since != 0 && modified_millis(&path) <= changed_since {
            continue;
        }
        // A file the server is writing may vanish between the walk and here.
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let name = format!(
            "{top}/{}",
            relative
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/")
        );
        zip.start_file(name, options).map_err(io::Error::other)?;
        zip.write_all(&bytes)?;
    }
    zip.finish().map_err(io::Error::other)?;
    std::fs::rename(&partial, target)?;
    Ok(std::fs::metadata(target)?.len())
}
