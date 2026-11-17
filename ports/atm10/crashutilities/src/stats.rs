//! The pure counting behind `/cu entities` and `/cu loadedchunks`.
use std::collections::HashMap;

/// Counts per key, largest first; ties by key so the output is stable.
pub fn ranked<K: Ord + Clone + std::hash::Hash>(
    items: impl IntoIterator<Item = K>,
) -> Vec<(K, usize)> {
    let mut counts: HashMap<K, usize> = HashMap::new();
    for item in items {
        *counts.entry(item).or_default() += 1;
    }
    let mut rows: Vec<_> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows
}

/// The chunk `[x, z]` a position stands in.
pub fn chunk_of(pos: [f64; 3]) -> [i32; 2] {
    [(pos[0].floor() as i32) >> 4, (pos[2].floor() as i32) >> 4]
}

/// The resident set size in megabytes from a Linux `/proc/self/status` text.
pub fn rss_megabytes(status: &str) -> Option<u64> {
    let line = status.lines().find(|l| l.starts_with("VmRSS:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024)
}

/// Whether the ground-item clear runs now: enabled, on its interval, and over the limit.
pub fn should_clear(config: &crate::config::ItemClear, tick: u64, items: usize) -> bool {
    config.enabled
        && config.interval_ticks > 0
        && tick.is_multiple_of(config.interval_ticks)
        && items > config.maximum
}
