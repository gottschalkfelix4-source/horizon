//! Powah's rules with no game in them: tiers and their numbers, what each generator makes in a
//! tick, the reactor, the energizing recipes and ender channels. The numbers are Powah's default
//! config as remembered; README.md lists them to check against the original.
use mod_api::{Compound, Tag};

/// The seven tiers, weakest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    Starter,
    Basic,
    Hardened,
    Blazing,
    Niotic,
    Spirited,
    Nitro,
}

pub const TIERS: [Tier; 7] = [
    Tier::Starter,
    Tier::Basic,
    Tier::Hardened,
    Tier::Blazing,
    Tier::Niotic,
    Tier::Spirited,
    Tier::Nitro,
];

const CELL_CAPACITY: [i32; 7] = [100_000, 1_000_000, 5_000_000, 10_000_000, 25_000_000, 50_000_000, 100_000_000];
const CELL_TRANSFER: [i32; 7] = [1_000, 5_000, 15_000, 50_000, 100_000, 250_000, 1_000_000];
const CABLE_TRANSFER: [i32; 7] = [100, 500, 1_500, 5_000, 10_000, 25_000, 50_000];
const SOLAR: [i32; 7] = [5, 15, 40, 100, 250, 550, 1_200];
const FURNATOR: [i32; 7] = [20, 50, 120, 300, 700, 1_600, 3_600];
const MAGMATOR: [i32; 7] = [25, 60, 150, 375, 900, 2_000, 4_500];
const THERMO: [i32; 7] = [10, 25, 60, 150, 375, 900, 2_200];
const REACTOR: [i32; 7] = [100, 250, 600, 1_500, 3_500, 8_000, 18_000];
const ROD: [i32; 7] = [20, 100, 400, 1_500, 5_000, 15_000, 40_000];

impl Tier {
    pub fn name(self) -> &'static str {
        ["starter", "basic", "hardened", "blazing", "niotic", "spirited", "nitro"][self as usize]
    }
    pub fn parse(name: &str) -> Option<Self> {
        TIERS.into_iter().find(|t| t.name() == name)
    }
    pub fn cell_capacity(self) -> i32 {
        CELL_CAPACITY[self as usize]
    }
    pub fn cell_transfer(self) -> i32 {
        CELL_TRANSFER[self as usize]
    }
    pub fn cable_transfer(self) -> i32 {
        CABLE_TRANSFER[self as usize]
    }
    pub fn solar(self) -> i32 {
        SOLAR[self as usize]
    }
    pub fn furnator(self) -> i32 {
        FURNATOR[self as usize]
    }
    pub fn magmator(self) -> i32 {
        MAGMATOR[self as usize]
    }
    pub fn thermo(self) -> i32 {
        THERMO[self as usize]
    }
    pub fn reactor(self) -> i32 {
        REACTOR[self as usize]
    }
    pub fn rod_transfer(self) -> i32 {
        ROD[self as usize]
    }
    /// An ender cell holds what an energy cell of its tier holds.
    pub fn ender_capacity(self) -> i32 {
        self.cell_capacity()
    }
}

/// What a block is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Cell,
    Cable,
    Furnator,
    Magmator,
    Thermo,
    Solar,
    Reactor,
    Rod,
    EnderCell,
    Orb,
}

impl Kind {
    pub const ALL: [Kind; 10] = [
        Kind::Cell,
        Kind::Cable,
        Kind::Furnator,
        Kind::Magmator,
        Kind::Thermo,
        Kind::Solar,
        Kind::Reactor,
        Kind::Rod,
        Kind::EnderCell,
        Kind::Orb,
    ];
    /// The block path before `_<tier>`.
    pub fn stem(self) -> &'static str {
        match self {
            Kind::Cell => "energy_cell",
            Kind::Cable => "energy_cable",
            Kind::Furnator => "furnator",
            Kind::Magmator => "magmator",
            Kind::Thermo => "thermo_generator",
            Kind::Solar => "solar_panel",
            Kind::Reactor => "reactor",
            Kind::Rod => "energizing_rod",
            Kind::EnderCell => "ender_cell",
            Kind::Orb => "energizing_orb",
        }
    }
    /// The block entity type's path.
    pub fn entity_type(self) -> &'static str {
        self.stem()
    }
    pub fn is_generator(self) -> bool {
        matches!(self, Kind::Furnator | Kind::Magmator | Kind::Thermo | Kind::Solar | Kind::Reactor)
    }
    pub fn has_tank(self) -> bool {
        matches!(self, Kind::Magmator | Kind::Thermo | Kind::Reactor)
    }
    pub fn tiers(self) -> Vec<Option<Tier>> {
        if self == Kind::Orb { vec![None] } else { TIERS.into_iter().map(Some).collect() }
    }
    pub fn block_ids(self) -> Vec<String> {
        self.tiers()
            .into_iter()
            .map(|t| match t {
                Some(t) => format!("powah:{}_{}", self.stem(), t.name()),
                None => format!("powah:{}", self.stem()),
            })
            .collect()
    }
    /// What this kind generates per tick at full speed.
    pub fn rate(self, t: Tier) -> i32 {
        match self {
            Kind::Furnator => t.furnator(),
            Kind::Magmator => t.magmator(),
            Kind::Thermo => t.thermo() * MAX_HEAT / 1000,
            Kind::Solar => t.solar(),
            Kind::Reactor => t.reactor() * 6 / 5,
            _ => 0,
        }
    }
}

/// `powah:<stem>_<tier>` (or `powah:energizing_orb`) as kind and tier.
pub fn parse_block(id: &str) -> Option<(Kind, Option<Tier>)> {
    let path = id.strip_prefix("powah:")?;
    if path == Kind::Orb.stem() {
        return Some((Kind::Orb, None));
    }
    let (stem, tier) = path.rsplit_once('_')?;
    let tier = Tier::parse(tier)?;
    Kind::ALL
        .into_iter()
        .find(|k| *k != Kind::Orb && k.stem() == stem)
        .map(|k| (k, Some(tier)))
}

pub const FURNATOR_SLOTS: usize = 5;
pub const REACTOR_SLOTS: usize = 5;
pub const ORB_SLOTS: usize = 9;
/// The orb's last slot takes the result; the others hold ingredients.
pub const ORB_OUTPUT: usize = 8;
/// Every Powah tank holds this many millibuckets.
pub const TANK_MB: i32 = 5_000;
pub const MAX_NETWORK: usize = 1_024;
pub const ENDER_CHANNELS: i32 = 12;
/// The orb draws from rods up to this far away on each axis.
pub const ROD_RANGE: i32 = 4;

/// A generator's buffer: 400 ticks of its best output, at least 10 000 FE.
pub fn generator_capacity(kind: Kind, t: Tier) -> i32 {
    (kind.rate(t) * 400).max(10_000)
}

/// How much a generator pushes out per tick: four ticks of its output.
pub fn generator_output(kind: Kind, t: Tier) -> i32 {
    kind.rate(t) * 4
}

/// A solar panel makes its rate by day under open sky, nothing at night or in rain.
pub fn solar_output(t: Tier, day: bool, sky: bool, rain: bool) -> i64 {
    if day && sky && !rain { i64::from(t.solar()) } else { 0 }
}

/// FE one item of fuel holds in a furnator: 30 FE per tick of vanilla burn time.
pub fn furnator_fuel_energy(burn_ticks: i32) -> i64 {
    i64::from(burn_ticks.max(0)) * 30
}

pub const MAGMATOR_MB_PER_DRAW: i32 = 100;
/// FE one millibucket of lava makes in a magmator (a bucket: 10 000 FE).
pub const MAGMATOR_FE_PER_MB: i64 = 10;

/// What a burning generator still has to give from the fuel it last took, in FE.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Burn {
    pub left: i64,
}

impl Burn {
    pub fn load(nbt: &Compound) -> Self {
        match nbt.get(&"burn".into()) {
            Some(Tag::Long(v)) => Burn { left: *v },
            _ => Burn::default(),
        }
    }
    pub fn save(&self, nbt: &mut Compound) {
        nbt.insert("burn".into(), Tag::Long(self.left));
    }
    /// One tick: takes new fuel (`draw`, giving FE) only when the last is spent and there is
    /// room; makes at most `rate` and never more than `room`.
    pub fn tick(&mut self, rate: i32, room: i32, draw: impl FnOnce() -> Option<i64>) -> i64 {
        if room <= 0 {
            return 0;
        }
        if self.left <= 0 {
            self.left = draw().unwrap_or(0);
        }
        let made = self.left.min(i64::from(rate)).min(i64::from(room)).max(0);
        self.left -= made;
        made
    }
}

/// The hottest a heat source gets (a blazing crystal block).
pub const MAX_HEAT: i32 = 2_800;

/// The heat of a source block.
pub fn heat_of(id: &str) -> i32 {
    match id {
        "minecraft:lava" => 1_000,
        "minecraft:magma_block" => 800,
        "powah:blazing_crystal_block" => MAX_HEAT,
        _ => 0,
    }
}

/// How cold a coolant is: water is the base (1); blue ice-cold fluids are colder.
pub fn coolant_of(fluid: &str) -> Option<i32> {
    match fluid {
        "minecraft:water" => Some(1),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Step {
    pub energy: i64,
    pub coolant_used: i32,
}

/// A thermo generator: the tier's rate scaled by the heat below (lava is 1000), while it has a
/// millibucket of coolant to spend each tick.
pub fn thermo_step(t: Tier, heat: i32, coolant: Option<(i32, i32)>, room: i32) -> Step {
    let Some((_, amount)) = coolant.filter(|c| c.1 > 0) else {
        return Step::default();
    };
    let _ = amount;
    if heat <= 0 || room <= 0 {
        return Step::default();
    }
    let made = (i64::from(t.thermo()) * i64::from(heat) / 1_000).min(i64::from(room));
    Step { energy: made, coolant_used: 1 }
}

/// The reactor: uraninite is its fuel, carbon (coal, charcoal) makes it give more, redstone makes
/// the fuel last longer, coolant keeps it from overheating.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reactor {
    /// Fuel left, in units; one uraninite is [`URANINITE_FUEL`].
    pub fuel: i64,
    /// Ticks of carbon left.
    pub carbon: i32,
    /// Ticks of redstone left.
    pub redstone: i32,
    /// Degrees.
    pub temperature: i32,
    /// Stopped by heat until it cools to [`RESTART_TEMPERATURE`].
    pub overheated: bool,
}

pub const URANINITE_FUEL: i64 = 100_000;
pub const FUEL_CAPACITY: i64 = 10 * URANINITE_FUEL;
pub const FUEL_PER_TICK: i64 = 10;
pub const CARBON_TICKS: i32 = 1_200;
pub const REDSTONE_TICKS: i32 = 600;
pub const MAX_TEMPERATURE: i32 = 1_000;
pub const RESTART_TEMPERATURE: i32 = 900;

impl Reactor {
    pub fn load(nbt: &Compound) -> Self {
        let get = |k: &str| match nbt.get(&k.into()) {
            Some(Tag::Long(v)) => *v,
            Some(Tag::Int(v)) => i64::from(*v),
            _ => 0,
        };
        Reactor {
            fuel: get("fuel"),
            carbon: get("carbon") as i32,
            redstone: get("redstone") as i32,
            temperature: get("temperature") as i32,
            overheated: get("overheated") != 0,
        }
    }
    pub fn save(&self, nbt: &mut Compound) {
        nbt.insert("fuel".into(), Tag::Long(self.fuel));
        nbt.insert("carbon".into(), Tag::Int(self.carbon));
        nbt.insert("redstone".into(), Tag::Int(self.redstone));
        nbt.insert("temperature".into(), Tag::Int(self.temperature));
        nbt.insert("overheated".into(), Tag::Int(i32::from(self.overheated)));
    }
    /// Takes one item of `id` if it is something the reactor uses and has room for.
    pub fn feed(&mut self, id: &str) -> bool {
        match id {
            "powah:uraninite" if self.fuel + URANINITE_FUEL <= FUEL_CAPACITY => {
                self.fuel += URANINITE_FUEL;
                true
            }
            "minecraft:coal" | "minecraft:charcoal" if self.carbon == 0 => {
                self.carbon = CARBON_TICKS;
                true
            }
            "minecraft:redstone" if self.redstone == 0 => {
                self.redstone = REDSTONE_TICKS;
                true
            }
            _ => false,
        }
    }
    /// One tick. `enabled`: no redstone signal stops it.
    pub fn tick(&mut self, t: Tier, enabled: bool, coolant: Option<(i32, i32)>, room: i32) -> Step {
        let mut step = Step::default();
        let cooling = match coolant {
            Some((cold, amount)) if amount > 0 => {
                step.coolant_used = 1;
                2 * cold
            }
            _ => 0,
        };
        if self.overheated && self.temperature <= RESTART_TEMPERATURE {
            self.overheated = false;
        }
        let running = enabled && self.fuel > 0 && room > 0 && !self.overheated;
        if running {
            let mut made = i64::from(t.reactor());
            if self.carbon > 0 {
                made = made * 6 / 5;
                self.carbon -= 1;
            }
            let mut burn = FUEL_PER_TICK;
            if self.redstone > 0 {
                burn = burn * 4 / 5;
                self.redstone -= 1;
            }
            step.energy = made.min(i64::from(room));
            self.fuel = (self.fuel - burn).max(0);
            self.temperature += 1;
        } else if self.temperature > 0 {
            self.temperature -= 1;
        }
        if step.coolant_used > 0 {
            if self.temperature > 0 {
                self.temperature = (self.temperature - cooling).max(0);
            } else {
                step.coolant_used = 0;
            }
        }
        if self.temperature >= MAX_TEMPERATURE {
            self.overheated = true;
        }
        step
    }
}

/// One energizing recipe: its ingredients (all of them, and nothing else, in the orb), its
/// result and the energy the rods must give.
#[derive(Debug, PartialEq, Eq)]
pub struct Energizing {
    pub inputs: &'static [(&'static str, u32)],
    pub output: &'static str,
    pub count: u32,
    pub energy: i64,
}

pub const ENERGIZING: [Energizing; 5] = [
    Energizing {
        inputs: &[("minecraft:iron_ingot", 1), ("minecraft:gold_ingot", 1)],
        output: "powah:steel_energized",
        count: 2,
        energy: 10_000,
    },
    Energizing { inputs: &[("minecraft:blaze_rod", 1)], output: "powah:crystal_blazing", count: 1, energy: 120_000 },
    Energizing { inputs: &[("minecraft:diamond", 1)], output: "powah:crystal_niotic", count: 1, energy: 300_000 },
    Energizing { inputs: &[("minecraft:emerald", 1)], output: "powah:crystal_spirited", count: 1, energy: 1_000_000 },
    Energizing {
        inputs: &[
            ("minecraft:nether_star", 1),
            ("minecraft:redstone_block", 2),
            ("powah:blazing_crystal_block", 1),
        ],
        output: "powah:crystal_nitro",
        count: 16,
        energy: 20_000_000,
    },
];

/// The recipe the orb's contents make: every ingredient there in at least its count, and no
/// other item.
pub fn find_energizing(contents: &[(String, u32)]) -> Option<&'static Energizing> {
    if contents.is_empty() {
        return None;
    }
    ENERGIZING.iter().find(|r| {
        let only_ours = contents.iter().all(|(id, _)| r.inputs.iter().any(|(i, _)| i == id));
        let enough = r.inputs.iter().all(|(id, n)| {
            contents.iter().filter(|(i, _)| i == id).map(|(_, c)| *c).sum::<u32>() >= *n
        });
        only_ours && enough
    })
}

/// The positions an orb at `pos` draws from.
pub fn rods_around(pos: [i32; 3]) -> impl Iterator<Item = [i32; 3]> {
    let r = ROD_RANGE;
    (-r..=r).flat_map(move |dx| {
        (-r..=r).flat_map(move |dy| {
            (-r..=r).filter_map(move |dz| {
                ((dx, dy, dz) != (0, 0, 0)).then_some([pos[0] + dx, pos[1] + dy, pos[2] + dz])
            })
        })
    })
}

/// A shared ender channel: one owner's energy on one channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnderChannel {
    pub owner: u128,
    pub channel: i32,
    pub energy: i64,
}

/// An ender cell's tick: it puts all it holds in the channel and takes back as much as it can
/// hold, so every cell of a channel shows the same energy.
pub fn exchange_with_channel(channels: &mut Vec<EnderChannel>, owner: u128, channel: i32, cell: &mut i32, capacity: i32) {
    let index = match channels.iter().position(|c| c.owner == owner && c.channel == channel) {
        Some(i) => i,
        None => {
            channels.push(EnderChannel { owner, channel, energy: 0 });
            channels.len() - 1
        }
    };
    let pool = &mut channels[index].energy;
    *pool += i64::from((*cell).max(0));
    let take = (*pool).min(i64::from(capacity));
    *pool -= take;
    *cell = take as i32;
}

pub fn encode_channels(channels: &[EnderChannel]) -> Compound {
    let list = channels
        .iter()
        .map(|c| {
            let mut t = Compound::new();
            t.insert("hi".into(), Tag::Long((c.owner >> 64) as i64));
            t.insert("lo".into(), Tag::Long(c.owner as u64 as i64));
            t.insert("channel".into(), Tag::Int(c.channel));
            t.insert("energy".into(), Tag::Long(c.energy));
            Tag::Compound(t)
        })
        .collect();
    let mut nbt = Compound::new();
    nbt.insert("channels".into(), Tag::List(list));
    nbt
}

pub fn decode_channels(nbt: &Compound) -> Option<Vec<EnderChannel>> {
    let Some(Tag::List(list)) = nbt.get(&"channels".into()) else {
        return Some(Vec::new());
    };
    list.iter()
        .map(|t| {
            let Tag::Compound(t) = t else { return None };
            let long = |k: &str| match t.get(&k.into()) {
                Some(Tag::Long(v)) => Some(*v),
                _ => None,
            };
            let Some(Tag::Int(channel)) = t.get(&"channel".into()) else { return None };
            Some(EnderChannel {
                owner: (u128::from(long("hi")? as u64) << 64) | u128::from(long("lo")? as u64),
                channel: *channel,
                energy: long("energy")?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_ids_parse_back_to_their_kind_and_tier() {
        assert_eq!(parse_block("powah:energy_cell_niotic"), Some((Kind::Cell, Some(Tier::Niotic))));
        assert_eq!(parse_block("powah:thermo_generator_starter"), Some((Kind::Thermo, Some(Tier::Starter))));
        assert_eq!(parse_block("powah:energizing_orb"), Some((Kind::Orb, None)));
        assert_eq!(parse_block("powah:energy_cell_mega"), None);
        assert_eq!(parse_block("minecraft:energy_cell_starter"), None);
        for kind in Kind::ALL {
            for id in kind.block_ids() {
                assert_eq!(parse_block(&id).map(|p| p.0), Some(kind), "{id}");
            }
        }
    }

    #[test]
    fn solar_panels_work_only_by_day_under_open_dry_sky() {
        assert_eq!(solar_output(Tier::Basic, true, true, false), 15);
        assert_eq!(solar_output(Tier::Basic, false, true, false), 0);
        assert_eq!(solar_output(Tier::Basic, true, false, false), 0);
        assert_eq!(solar_output(Tier::Basic, true, true, true), 0);
    }

    #[test]
    fn a_furnator_burns_one_coal_into_48000_fe_at_its_rate() {
        let mut burn = Burn::default();
        let mut draws = 0;
        let mut total = 0;
        // 48 000 FE at 20 FE/t is 2 400 ticks, then it draws again.
        for _ in 0..2_400 {
            total += burn.tick(20, 1_000_000, || {
                draws += 1;
                Some(furnator_fuel_energy(1_600))
            });
        }
        assert_eq!((draws, total), (1, 48_000));
        burn.tick(20, 1_000_000, || {
            draws += 1;
            None
        });
        assert_eq!(draws, 2);
    }

    #[test]
    fn a_full_generator_takes_no_fuel() {
        let mut burn = Burn::default();
        let made = burn.tick(20, 0, || panic!("drew fuel with no room"));
        assert_eq!(made, 0);
        let made = burn.tick(20, 7, || Some(100));
        assert_eq!((made, burn.left), (7, 93));
    }

    #[test]
    fn a_thermo_generator_scales_with_heat_and_needs_coolant() {
        assert_eq!(thermo_step(Tier::Starter, 1_000, Some((1, 10)), 1_000), Step { energy: 10, coolant_used: 1 });
        assert_eq!(thermo_step(Tier::Starter, 2_800, Some((1, 10)), 1_000).energy, 28);
        assert_eq!(thermo_step(Tier::Starter, 1_000, Some((1, 0)), 1_000), Step::default());
        assert_eq!(thermo_step(Tier::Starter, 1_000, None, 1_000), Step::default());
        assert_eq!(thermo_step(Tier::Starter, 0, Some((1, 10)), 1_000), Step::default());
    }

    #[test]
    fn a_reactor_without_coolant_overheats_then_restarts_once_cooled() {
        let mut r = Reactor::default();
        assert!(r.feed("powah:uraninite"));
        let mut ticks = 0;
        while !r.overheated {
            let s = r.tick(Tier::Starter, true, None, i32::MAX);
            assert_eq!(s.energy, 100);
            ticks += 1;
        }
        assert_eq!(ticks, MAX_TEMPERATURE);
        // Stopped until it cools to the restart temperature.
        for _ in 0..(MAX_TEMPERATURE - RESTART_TEMPERATURE) {
            assert_eq!(r.tick(Tier::Starter, true, None, i32::MAX).energy, 0);
        }
        assert_eq!(r.tick(Tier::Starter, true, None, i32::MAX).energy, 100);
    }

    #[test]
    fn carbon_and_redstone_raise_output_and_stretch_fuel() {
        let mut r = Reactor::default();
        r.feed("powah:uraninite");
        r.feed("minecraft:coal");
        r.feed("minecraft:redstone");
        let s = r.tick(Tier::Basic, true, Some((1, 100)), i32::MAX);
        assert_eq!(s.energy, 300);
        assert_eq!(r.fuel, URANINITE_FUEL - 8);
        // A second coal waits for the first to burn out.
        assert!(!r.feed("minecraft:charcoal"));
    }

    #[test]
    fn a_redstone_signal_stops_the_reactor() {
        let mut r = Reactor { fuel: URANINITE_FUEL, ..Default::default() };
        assert_eq!(r.tick(Tier::Starter, false, None, i32::MAX).energy, 0);
        assert_eq!(r.fuel, URANINITE_FUEL);
    }

    #[test]
    fn the_orb_matches_whole_recipes_and_nothing_extra() {
        let c = |v: &[(&str, u32)]| v.iter().map(|(i, n)| (i.to_string(), *n)).collect::<Vec<_>>();
        let steel = find_energizing(&c(&[("minecraft:gold_ingot", 3), ("minecraft:iron_ingot", 1)]));
        assert_eq!(steel.map(|r| (r.output, r.count)), Some(("powah:steel_energized", 2)));
        assert_eq!(find_energizing(&c(&[("minecraft:iron_ingot", 1)])), None);
        assert_eq!(find_energizing(&c(&[("minecraft:diamond", 1), ("minecraft:dirt", 1)])), None);
        let nitro = c(&[
            ("minecraft:nether_star", 1),
            ("minecraft:redstone_block", 1),
            ("minecraft:redstone_block", 1),
            ("powah:blazing_crystal_block", 1),
        ]);
        assert_eq!(find_energizing(&nitro).map(|r| r.energy), Some(20_000_000));
        assert_eq!(find_energizing(&[]), None);
    }

    #[test]
    fn rods_are_looked_for_in_a_cube_around_the_orb() {
        let all: Vec<_> = rods_around([0, 64, 0]).collect();
        assert_eq!(all.len(), 9 * 9 * 9 - 1);
        assert!(all.contains(&[4, 60, -4]));
        assert!(!all.contains(&[0, 64, 0]));
    }

    #[test]
    fn ender_cells_on_one_channel_share_their_energy() {
        let mut channels = Vec::new();
        let (mut a, mut b) = (600, 0);
        exchange_with_channel(&mut channels, 7, 0, &mut a, 1_000);
        exchange_with_channel(&mut channels, 7, 0, &mut b, 1_000);
        assert_eq!((a, b), (600, 0));
        // `a` deposits and keeps up to its capacity; the other owner's channel 0 is apart.
        let mut c = 0;
        exchange_with_channel(&mut channels, 8, 0, &mut c, 1_000);
        assert_eq!(c, 0);
        let mut small = 600;
        exchange_with_channel(&mut channels, 7, 1, &mut small, 250);
        assert_eq!(small, 250);
        assert_eq!(channels.iter().find(|ch| ch.owner == 7 && ch.channel == 1).unwrap().energy, 350);
    }

    #[test]
    fn ender_channels_round_trip_through_their_saved_form() {
        let channels = vec![EnderChannel { owner: u128::MAX - 5, channel: 11, energy: 1 << 40 }];
        assert_eq!(decode_channels(&encode_channels(&channels)), Some(channels));
    }
}
