//! What a sushi roll is worth, apart from the game.

/// Nutrition of a roll of rice and nori alone.
pub const BASE_NUTRITION: u32 = 3;
/// The most a roll can feed.
pub const MAX_NUTRITION: u32 = 20;
/// Saturation per point of nutrition.
pub const SATURATION_PER_NUTRITION: f32 = 0.6;
/// Ticks the mat takes to roll one.
pub const ROLL_TICKS: i32 = 40;

/// What a filling adds, by item id.
pub fn filling(item: &str) -> Option<u32> {
    Some(match item {
        "minecraft:salmon" => 4,
        "minecraft:cod" => 3,
        "minecraft:tropical_fish" => 2,
        "minecraft:carrot" | "minecraft:dried_kelp" | "sushigocrafting:wasabi" | "sushigocrafting:soy_sauce" => 1,
        _ => return None,
    })
}

/// A roll's food value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Food {
    pub nutrition: u32,
    pub saturation: f32,
}

/// The roll the fillings make (at least one, at most two): rice and nori's 3 plus each filling,
/// capped at 20, with saturation 0.6 per point. No filling, no roll.
pub fn roll(fillings: &[&str]) -> Option<Food> {
    let values: Vec<u32> = fillings.iter().filter_map(|f| filling(f)).collect();
    if values.is_empty() || values.len() != fillings.len() {
        return None;
    }
    let nutrition = (BASE_NUTRITION + values.iter().sum::<u32>()).min(MAX_NUTRITION);
    Some(Food { nutrition, saturation: nutrition as f32 * SATURATION_PER_NUTRITION })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_roll_is_worth_three_plus_its_fillings_with_saturation_to_match() {
        let f = roll(&["minecraft:salmon", "minecraft:cod"]).unwrap();
        assert_eq!(f.nutrition, 10);
        assert!((f.saturation - 6.0).abs() < 1e-5);
        assert_eq!(roll(&["minecraft:carrot"]).unwrap().nutrition, 4);
    }

    #[test]
    fn a_roll_needs_a_filling_and_refuses_anything_that_is_not_one() {
        assert_eq!(roll(&[]), None);
        assert_eq!(roll(&["minecraft:salmon", "minecraft:stone"]), None);
    }
}
