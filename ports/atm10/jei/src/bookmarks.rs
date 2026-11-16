//! Bookmarks ("A" over an item): a list the player keeps beside the screen, saved between
//! sessions as one item id per line in JEI's data folder (`bookmarks.txt`).

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bookmarks {
    items: Vec<String>,
}

impl Bookmarks {
    pub const fn new() -> Self {
        Self { items: Vec::new() }
    }
    /// Bookmark `item`, or take its bookmark away when it has one (JEI's "A" toggles). Newly
    /// bookmarked items go first, as JEI adds them. True when it is now bookmarked.
    pub fn toggle(&mut self, item: &str) -> bool {
        if let Some(i) = self.items.iter().position(|b| b == item) {
            self.items.remove(i);
            false
        } else {
            self.items.insert(0, item.to_owned());
            true
        }
    }
    pub fn items(&self) -> &[String] {
        &self.items
    }
    pub fn contains(&self, item: &str) -> bool {
        self.items.iter().any(|b| b == item)
    }
    /// The saved form.
    pub fn save(&self) -> String {
        let mut out = self.items.join("\n");
        out.push('\n');
        out
    }
    /// Read the saved form; blank lines, duplicates and lines that are not item ids are skipped.
    pub fn load(text: &str) -> Self {
        let mut items: Vec<String> = Vec::new();
        for line in text.lines().map(str::trim) {
            let valid = line.split_once(':').is_some_and(|(ns, path)| {
                !ns.is_empty() && !path.is_empty() && !line.contains(char::is_whitespace)
            });
            if valid && !items.iter().any(|i| i == line) {
                items.push(line.to_owned());
            }
        }
        Self { items }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_toggles_and_new_bookmarks_go_first() {
        let mut b = Bookmarks::default();
        assert!(b.toggle("minecraft:diamond"));
        assert!(b.toggle("minecraft:stick"));
        assert_eq!(b.items(), ["minecraft:stick", "minecraft:diamond"]);
        assert!(!b.toggle("minecraft:diamond"));
        assert_eq!(b.items(), ["minecraft:stick"]);
    }

    #[test]
    fn bookmarks_survive_saving_and_skip_junk_lines() {
        let mut b = Bookmarks::default();
        b.toggle("minecraft:diamond");
        b.toggle("mekanism:ingot_osmium");
        assert_eq!(Bookmarks::load(&b.save()), b);
        let read = Bookmarks::load("minecraft:dirt\n\nnot an id\nminecraft:dirt\n:x\n");
        assert_eq!(read.items(), ["minecraft:dirt"]);
    }
}
