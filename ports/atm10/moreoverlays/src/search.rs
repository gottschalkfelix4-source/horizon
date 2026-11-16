//! Inventory search: Ctrl+F on a container screen opens a field; slots whose item does not match
//! are darkened.

/// Whether an item named `name` matches the search `query`: every word of the query appears in
/// the name, ignoring case. An empty query matches everything.
pub fn matches(query: &str, name: &str) -> bool {
    let name = name.to_lowercase();
    query
        .split_whitespace()
        .all(|word| name.contains(&word.to_lowercase()))
}

/// Whether a search with this query darkens anything at all.
pub fn active(query: &str) -> bool {
    !query.trim().is_empty()
}

/// The colour drawn over a slot whose item does not match, 0xAARRGGBB.
pub const DIM: u32 = 0xC000_0000;

/// Where the search field goes for a container panel `[x, y, width, height]`: centred under it.
pub fn field_rect(panel: [f32; 4]) -> [f32; 4] {
    let width = 120.0_f32.min(panel[2]);
    [
        panel[0] + (panel[2] - width) / 2.0,
        panel[1] + panel[3] + 4.0,
        width,
        16.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_must_appear_in_the_name_whatever_the_case() {
        assert!(matches("iron", "Iron Ingot"));
        assert!(matches("INGOT ir", "Iron Ingot"));
        assert!(!matches("gold ingot", "Iron Ingot"));
        assert!(matches("", "Dirt"));
        assert!(!active("   "));
    }
}
