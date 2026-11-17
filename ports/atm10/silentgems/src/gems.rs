//! The gem catalogue and the blocks and items each gem gets: pure data, no game access.

/// Where a gem's ore generates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GemSet {
    /// The Overworld (stone and deepslate ores).
    Classic,
    /// The Nether (netherrack ores).
    Dark,
    /// The End (end stone ores).
    Light,
}

/// The 48 gems, 16 per set (names from the mod's public gem list; to check against the jar).
pub const GEMS: [(&str, GemSet); 48] = {
    use GemSet::*;
    [
        ("ruby", Classic),
        ("garnet", Classic),
        ("topaz", Classic),
        ("amber", Classic),
        ("heliodor", Classic),
        ("peridot", Classic),
        ("green_sapphire", Classic),
        ("phosphophyllite", Classic),
        ("aquamarine", Classic),
        ("sapphire", Classic),
        ("tanzanite", Classic),
        ("amethyst", Classic),
        ("agate", Classic),
        ("morganite", Classic),
        ("onyx", Classic),
        ("opal", Classic),
        ("carnelian", Dark),
        ("spinel", Dark),
        ("citrine", Dark),
        ("jasper", Dark),
        ("zircon", Dark),
        ("moldavite", Dark),
        ("malachite", Dark),
        ("turquoise", Dark),
        ("euclase", Dark),
        ("benitoite", Dark),
        ("iolite", Dark),
        ("alexandrite", Dark),
        ("lepidolite", Dark),
        ("ametrine", Dark),
        ("black_diamond", Dark),
        ("moonstone", Dark),
        ("pyrope", Light),
        ("coral", Light),
        ("sunstone", Light),
        ("cats_eye", Light),
        ("yellow_diamond", Light),
        ("jade", Light),
        ("chrysoprase", Light),
        ("apatite", Light),
        ("fluorite", Light),
        ("kyanite", Light),
        ("sodalite", Light),
        ("ammolite", Light),
        ("kunzite", Light),
        ("rose_quartz", Light),
        ("tektite", Light),
        ("pearl", Light),
    ]
};

/// One block declaration: id, vanilla template, light level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockDecl {
    pub id: String,
    pub template: &'static str,
    pub light: Option<u8>,
}

/// One item declaration: id, vanilla template, stack size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemDecl {
    pub id: String,
    pub template: &'static str,
    pub max_stack_size: Option<u8>,
}

/// Light a glowrose gives (assumed).
pub const GLOWROSE_LIGHT: u8 = 10;

fn ore_template(set: GemSet) -> &'static str {
    match set {
        GemSet::Classic => "minecraft:diamond_ore",
        GemSet::Dark => "minecraft:nether_quartz_ore",
        GemSet::Light => "minecraft:diamond_ore",
    }
}

/// Every gem block: ores (a deepslate one for the Overworld set), the storage block, bricks,
/// glass, the glowrose and the teleporter.
pub fn blocks() -> Vec<BlockDecl> {
    let mut out = Vec::new();
    let block = |id: String, template: &'static str, light: Option<u8>| BlockDecl {
        id,
        template,
        light,
    };
    for (gem, set) in GEMS {
        out.push(block(
            format!("silentgems:{gem}_ore"),
            ore_template(set),
            None,
        ));
        if set == GemSet::Classic {
            out.push(block(
                format!("silentgems:deepslate_{gem}_ore"),
                "minecraft:deepslate_diamond_ore",
                None,
            ));
        }
        out.push(block(
            format!("silentgems:{gem}_block"),
            "minecraft:diamond_block",
            None,
        ));
        out.push(block(
            format!("silentgems:{gem}_bricks"),
            "minecraft:stone_bricks",
            None,
        ));
        out.push(block(
            format!("silentgems:{gem}_glass"),
            "minecraft:glass",
            None,
        ));
        out.push(block(
            format!("silentgems:{gem}_glowrose"),
            "minecraft:poppy",
            Some(GLOWROSE_LIGHT),
        ));
        out.push(block(teleporter_id(gem), "minecraft:iron_block", None));
    }
    out
}

/// Every item that is not a block's: the gems, their return home charms, the linker.
pub fn items() -> Vec<ItemDecl> {
    let mut out: Vec<ItemDecl> = GEMS
        .iter()
        .flat_map(|(gem, _)| {
            [
                ItemDecl {
                    id: format!("silentgems:{gem}"),
                    template: "minecraft:diamond",
                    max_stack_size: None,
                },
                ItemDecl {
                    id: charm_id(gem),
                    template: "minecraft:iron_ingot",
                    max_stack_size: Some(1),
                },
            ]
        })
        .collect();
    out.push(ItemDecl {
        id: LINKER.to_string(),
        template: "minecraft:iron_ingot",
        max_stack_size: Some(1),
    });
    out
}

pub const LINKER: &str = "silentgems:teleporter_linker";

pub fn teleporter_id(gem: &str) -> String {
    format!("silentgems:{gem}_teleporter")
}

pub fn charm_id(gem: &str) -> String {
    format!("silentgems:{gem}_return_home_charm")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_gem_gets_its_blocks_and_ids_never_clash() {
        let blocks = blocks();
        // Six blocks a gem, plus a deepslate ore for each of the 16 Overworld gems.
        assert_eq!(blocks.len(), 48 * 6 + 16);
        let items = items();
        assert_eq!(items.len(), 48 * 2 + 1);
        let ids: BTreeSet<&str> = blocks
            .iter()
            .map(|b| b.id.as_str())
            .chain(items.iter().map(|i| i.id.as_str()))
            .collect();
        assert_eq!(ids.len(), blocks.len() + items.len());
    }

    #[test]
    fn ores_follow_their_set_dimension() {
        let blocks = blocks();
        let template = |id: &str| blocks.iter().find(|b| b.id == id).map(|b| b.template);
        assert_eq!(
            template("silentgems:ruby_ore"),
            Some("minecraft:diamond_ore")
        );
        assert!(template("silentgems:deepslate_ruby_ore").is_some());
        assert_eq!(
            template("silentgems:carnelian_ore"),
            Some("minecraft:nether_quartz_ore")
        );
        assert_eq!(template("silentgems:deepslate_carnelian_ore"), None);
    }
}
