//! Mekanism Generators' numbers, free of the game so they can be tested. All from the default
//! config as remembered; README.md lists them under "To check". Mekanism counts joules (J);
//! the pack's unit is FE, 1 FE = 2.5 J.

pub fn fe(joules: i64) -> i32 {
    (joules as f64 / 2.5) as i32
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gen {
    Heat,
    Solar,
    AdvancedSolar,
    Wind,
    Bio,
}

impl Gen {
    pub const ALL: [Gen; 5] = [Gen::Heat, Gen::Solar, Gen::AdvancedSolar, Gen::Wind, Gen::Bio];

    pub fn name(self) -> &'static str {
        match self {
            Gen::Heat => "heat_generator",
            Gen::Solar => "solar_generator",
            Gen::AdvancedSolar => "advanced_solar_generator",
            Gen::Wind => "wind_generator",
            Gen::Bio => "bio_generator",
        }
    }

    pub fn id(self) -> String {
        format!("mekanismgenerators:{}", self.name())
    }

    pub fn from_id(id: &str) -> Option<Gen> {
        let name = id.strip_prefix("mekanismgenerators:")?;
        Gen::ALL.into_iter().find(|g| g.name() == name)
    }

    /// FE per tick at full output.
    pub fn peak(self) -> i32 {
        fe(match self {
            Gen::Heat => 200,
            Gen::Solar => 50,
            Gen::AdvancedSolar => 300,
            Gen::Wind => 480,
            Gen::Bio => 350,
        })
    }

    /// The energy buffer, in FE: 100 ticks at peak, at least 80 kJ's worth.
    pub fn buffer(self) -> i32 {
        (self.peak() * 100).max(fe(80_000))
    }

    /// What it can hand on per tick: four times what it makes.
    pub fn rate(self) -> i32 {
        self.peak() * 4
    }

    /// Whether it burns fuel items.
    pub fn burns(self) -> bool {
        matches!(self, Gen::Heat | Gen::Bio)
    }
}

/// A solar panel's output this tick: full by day under open sky, none at night or in rain.
pub fn solar_output(g: Gen, day: bool, sky: bool, rain: bool) -> i32 {
    if matches!(g, Gen::Solar | Gen::AdvancedSolar) && day && sky && !rain { g.peak() } else { 0 }
}

/// A wind generator's output at height `y`: nothing below 24, rising to the peak at 319.
pub fn wind_output(y: i32) -> i32 {
    let (lo, hi) = (24, 319);
    let t = ((y - lo) as f64 / (hi - lo) as f64).clamp(0.0, 1.0);
    (Gen::Wind.peak() as f64 * t) as i32
}

/// How many ticks a fuel item burns in `g`, from the item's id and its vanilla burn time.
pub fn burn_ticks(g: Gen, item: &str, vanilla: i32) -> i32 {
    match g {
        Gen::Heat => vanilla.max(0),
        Gen::Bio => match item {
            "mekanism:bio_fuel" => 200,
            "minecraft:wheat_seeds" | "minecraft:beetroot_seeds" | "minecraft:melon_seeds"
            | "minecraft:pumpkin_seeds" | "minecraft:wheat" | "minecraft:sugar_cane"
            | "minecraft:carrot" | "minecraft:potato" | "minecraft:beetroot" => 100,
            _ => 0,
        },
        _ => 0,
    }
}

/// The fuel state: ticks of burn left.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Burn {
    pub left: i32,
}

impl Burn {
    /// One tick: what is made, taking a fuel (its burn ticks) when the last one ran out and there
    /// is room for the energy.
    pub fn tick(&mut self, g: Gen, room: i32, mut take_fuel: impl FnMut() -> Option<i32>) -> i32 {
        if self.left <= 0 && room >= g.peak() {
            if let Some(ticks) = take_fuel() {
                self.left = ticks;
            }
        }
        if self.left > 0 && room >= g.peak() {
            self.left -= 1;
            return g.peak();
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solar_makes_power_only_by_day_under_clear_sky() {
        assert_eq!(solar_output(Gen::Solar, true, true, false), 20);
        assert_eq!(solar_output(Gen::AdvancedSolar, true, true, false), 120);
        assert_eq!(solar_output(Gen::Solar, false, true, false), 0);
        assert_eq!(solar_output(Gen::Solar, true, false, false), 0);
        assert_eq!(solar_output(Gen::Solar, true, true, true), 0);
        assert_eq!(solar_output(Gen::Wind, true, true, false), 0);
    }

    #[test]
    fn wind_rises_with_height() {
        assert_eq!(wind_output(0), 0);
        assert_eq!(wind_output(24), 0);
        assert!(wind_output(100) > 0 && wind_output(100) < wind_output(200));
        assert_eq!(wind_output(400), Gen::Wind.peak());
    }

    #[test]
    fn a_burner_takes_fuel_only_with_room_and_burns_it_whole() {
        let mut burn = Burn::default();
        let mut fuels = 3;
        let mut made = 0;
        for _ in 0..700 {
            made += burn.tick(Gen::Bio, 10_000, || {
                (fuels > 0).then(|| {
                    fuels -= 1;
                    burn_ticks(Gen::Bio, "mekanism:bio_fuel", 0)
                })
            });
        }
        assert_eq!(fuels, 0);
        assert_eq!(made, 3 * 200 * Gen::Bio.peak());
        let mut full = Burn::default();
        assert_eq!(full.tick(Gen::Heat, 0, || Some(100)), 0);
        assert_eq!(full.left, 0);
    }

    #[test]
    fn ids_round_trip_and_bio_ignores_coal() {
        assert_eq!(Gen::from_id("mekanismgenerators:wind_generator"), Some(Gen::Wind));
        assert_eq!(Gen::from_id("mekanism:crusher"), None);
        assert_eq!(burn_ticks(Gen::Bio, "minecraft:coal", 1600), 0);
        assert_eq!(burn_ticks(Gen::Heat, "minecraft:coal", 1600), 1600);
    }
}
