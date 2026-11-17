//! Just Dire Things' rules with no game in them: the goo's spread (the original's `goospread` recipes
//! give what turns into what, for how long and from which tier) and the coal generator's burn.
use mod_api::serde_json::Value;

/// A goo recipe: the goo of `tier` or higher turns an adjacent `input` block into `output`.
#[derive(Clone, Debug, PartialEq)]
pub struct Goo {
    pub input: String,
    pub output: String,
    pub output_properties: Vec<(String, String)>,
    /// Ticks the conversion takes.
    pub duration: i64,
    pub tier: u32,
}

pub fn parse_goo(json: &Value) -> Option<Goo> {
    let out = json.get("output")?;
    let properties = out
        .get("Properties")
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default();
    Some(Goo {
        input: json.get("input")?.get("Name")?.as_str()?.to_owned(),
        output: out.get("Name")?.as_str()?.to_owned(),
        output_properties: properties,
        duration: json.get("crafting_duration")?.as_i64()?,
        tier: u32::try_from(json.get("tier_requirement")?.as_i64()?).ok()?,
    })
}

/// The tier of a goo block (`gooblock_tier1` to `gooblock_tier4`).
pub fn goo_tier(block: &str) -> Option<u32> {
    let n = block.strip_prefix("justdirethings:gooblock_tier")?;
    n.parse().ok().filter(|t| (1..=4).contains(t))
}

/// The recipe a goo block of `tier` runs on a neighbour: the one for that block whose tier it meets.
pub fn pick<'a>(recipes: &'a [Goo], neighbour: &str, tier: u32) -> Option<&'a Goo> {
    recipes
        .iter()
        .find(|r| r.input == neighbour && r.tier <= tier)
}

/// A goo block's progress: which of the six sides it works on and how far.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GooState {
    pub side: usize,
    pub progress: i64,
}

/// One tick of a goo block of `tier`. `neighbours` are the six blocks around it. Returns the side and
/// the recipe when a conversion finishes. Progress runs on one side; when that side's block stops
/// being convertible the goo moves to the next one that is, from zero.
pub fn goo_tick<'a>(
    state: &mut GooState,
    neighbours: &[Option<String>; 6],
    recipes: &'a [Goo],
    tier: u32,
) -> Option<(usize, &'a Goo)> {
    let on = |side: usize| {
        neighbours[side]
            .as_deref()
            .and_then(|b| pick(recipes, b, tier))
    };
    if on(state.side).is_none() {
        *state = GooState::default();
        state.side = (0..6).find(|s| on(*s).is_some())?;
    }
    let recipe = on(state.side)?;
    state.progress += 1;
    if state.progress >= recipe.duration {
        let side = state.side;
        *state = GooState::default();
        return Some((side, recipe));
    }
    None
}

/// Energy a coal generator makes per tick while it burns (assumed: the original's number is not known).
pub const GENERATOR_FE_PER_TICK: i32 = 40;
pub const GENERATOR_CAPACITY: i32 = 50_000;
pub const GENERATOR_PUSH: i32 = 1_000;

/// One tick of a generator holding `burn` ticks of fuel with room for `room` FE: the energy made, the
/// burn left and whether a new fuel item is wanted (burn is out and the buffer has room).
pub fn generator_tick(burn: i32, room: i32) -> (i32, i32, bool) {
    if burn > 0 && room >= GENERATOR_FE_PER_TICK {
        (GENERATOR_FE_PER_TICK, burn - 1, false)
    } else {
        (0, burn.max(0), burn <= 0 && room >= GENERATOR_FE_PER_TICK)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe(file: &str) -> Goo {
        let path = format!(
            "{}/data/justdirethings/recipe/{file}.json",
            env!("CARGO_MANIFEST_DIR")
        );
        parse_goo(&mod_api::serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap())
            .unwrap()
    }

    #[test]
    fn the_originals_goo_recipes_read_their_input_output_time_and_tier() {
        let r = recipe("raw_celestigem_ore-goospread");
        assert_eq!(r.input, "minecraft:diamond_block");
        assert_eq!(r.output, "justdirethings:raw_celestigem_ore");
        assert_eq!(
            r.output_properties,
            vec![("facing".to_owned(), "north".to_owned())]
        );
        assert_eq!((r.duration, r.tier), (4800, 3));
        assert_eq!(goo_tier("justdirethings:gooblock_tier3"), Some(3));
        assert_eq!(goo_tier("justdirethings:goosoil_tier3"), None);
    }

    #[test]
    fn a_goo_converts_a_neighbour_after_its_time_and_only_if_its_tier_is_high_enough() {
        let recipes = [
            recipe("raw_ferricore_ore-goospread"),
            recipe("raw_blazegold_ore-goospread"),
        ];
        let mut around: [Option<String>; 6] = Default::default();
        around[4] = Some("minecraft:iron_block".into());
        let mut state = GooState::default();
        for _ in 0..2399 {
            assert!(goo_tick(&mut state, &around, &recipes, 1).is_none());
        }
        let (side, done) = goo_tick(&mut state, &around, &recipes, 1).unwrap();
        assert_eq!(
            (side, done.output.as_str()),
            (4, "justdirethings:raw_ferricore_ore")
        );
        // gold needs tier 2: tier 1 goo never finishes it
        around[4] = Some("minecraft:gold_block".into());
        let mut state = GooState::default();
        assert!((0..5000).all(|_| goo_tick(&mut state, &around, &recipes, 1).is_none()));
        assert!(
            (0..2400)
                .filter_map(|_| goo_tick(&mut state, &around, &recipes, 2))
                .count()
                == 1
        );
    }

    #[test]
    fn removing_the_block_mid_conversion_starts_the_goo_over() {
        let recipes = [recipe("raw_ferricore_ore-goospread")];
        let mut around: [Option<String>; 6] = Default::default();
        around[0] = Some("minecraft:iron_block".into());
        let mut state = GooState::default();
        for _ in 0..1000 {
            goo_tick(&mut state, &around, &recipes, 1);
        }
        around[0] = None;
        goo_tick(&mut state, &around, &recipes, 1);
        assert_eq!(state, GooState::default());
    }

    #[test]
    fn a_generator_burns_while_it_has_room_and_asks_for_fuel_when_out() {
        assert_eq!(generator_tick(10, 1000), (40, 9, false));
        assert_eq!(generator_tick(10, 10), (0, 10, false));
        assert_eq!(generator_tick(0, 1000), (0, 0, true));
    }
}
