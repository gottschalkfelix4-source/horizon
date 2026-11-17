//! AE2's numbers and the network model, free of the game so they can be tested. The figures are
//! the default config as remembered; README.md lists what a check against the original must confirm.

/// AE2's power unit (AE) against FE: 1 AE = 2 FE in the pack's default config.
pub const FE_PER_AE: i32 = 2;

/// Most blocks one network scan visits.
pub const MAX_NODES: usize = 8192;

/// The cable colours: `fluix` is the uncoloured cable, then the sixteen dyes.
pub const COLORS: [&str; 17] = [
    "fluix", "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray",
    "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cable {
    Glass,
    Covered,
    Smart,
    CoveredDense,
    SmartDense,
}

impl Cable {
    pub const ALL: [Cable; 5] =
        [Cable::Glass, Cable::Covered, Cable::Smart, Cable::CoveredDense, Cable::SmartDense];

    pub fn name(self) -> &'static str {
        match self {
            Cable::Glass => "glass_cable",
            Cable::Covered => "covered_cable",
            Cable::Smart => "smart_cable",
            Cable::CoveredDense => "covered_dense_cable",
            Cable::SmartDense => "smart_dense_cable",
        }
    }

    /// Channels one cable carries (8, or 32 for dense cables).
    pub fn channels(self) -> u32 {
        match self {
            Cable::CoveredDense | Cable::SmartDense => 32,
            _ => 8,
        }
    }

    /// Idle power in hundredths of an FE per tick (0.1 AE for smart cables, 0.4 for dense smart).
    pub fn idle_centi_fe(self) -> i64 {
        match self {
            Cable::Smart => 20,
            Cable::SmartDense => 80,
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Device {
    Controller,
    EnergyAcceptor,
    EnergyCell,
    DenseEnergyCell,
    Drive,
    Terminal,
    ImportBus,
    ExportBus,
    PatternProvider,
    MolecularAssembler,
}

impl Device {
    pub const ALL: [Device; 10] = [
        Device::Controller,
        Device::EnergyAcceptor,
        Device::EnergyCell,
        Device::DenseEnergyCell,
        Device::Drive,
        Device::Terminal,
        Device::ImportBus,
        Device::ExportBus,
        Device::PatternProvider,
        Device::MolecularAssembler,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Device::Controller => "controller",
            Device::EnergyAcceptor => "energy_acceptor",
            Device::EnergyCell => "energy_cell",
            Device::DenseEnergyCell => "dense_energy_cell",
            Device::Drive => "drive",
            Device::Terminal => "terminal",
            Device::ImportBus => "import_bus",
            Device::ExportBus => "export_bus",
            Device::PatternProvider => "pattern_provider",
            Device::MolecularAssembler => "molecular_assembler",
        }
    }

    /// FE a block holds: cells 200k and 1.6M AE, the acceptor a small buffer to hand on.
    pub fn capacity_fe(self) -> i32 {
        match self {
            Device::Controller
            | Device::Drive
            | Device::Terminal
            | Device::ImportBus
            | Device::ExportBus
            | Device::PatternProvider
            | Device::MolecularAssembler => 0,
            Device::EnergyAcceptor => 800,
            Device::EnergyCell => 200_000 * FE_PER_AE,
            Device::DenseEnergyCell => 1_600_000 * FE_PER_AE,
        }
    }

    /// Idle power in hundredths of an FE per tick (the controller draws 6 AE per tick).
    pub fn idle_centi_fe(self) -> i64 {
        match self {
            Device::Controller => 6 * FE_PER_AE as i64 * 100,
            // the terminal draws 0.5 AE per tick
            Device::Terminal => 100,
            // buses draw 0.1 AE per tick
            Device::ImportBus | Device::ExportBus => 20,
            // the provider draws 0.1 AE, the assembler 2 AE per tick
            Device::PatternProvider => 20,
            Device::MolecularAssembler => 400,
            _ => 0,
        }
    }

    /// Whether the device takes one of the network's channels to work.
    pub fn needs_channel(self) -> bool {
        matches!(
            self,
            Device::Drive
                | Device::Terminal
                | Device::ImportBus
                | Device::ExportBus
                | Device::PatternProvider
                | Device::MolecularAssembler
        )
    }

    pub fn is_bus(self) -> bool {
        matches!(self, Device::ImportBus | Device::ExportBus)
    }

    pub fn is_cell(self) -> bool {
        matches!(self, Device::EnergyCell | Device::DenseEnergyCell)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Cable(Cable),
    Device(Device),
}

impl Kind {
    pub fn idle_centi_fe(self) -> i64 {
        match self {
            Kind::Cable(c) => c.idle_centi_fe(),
            Kind::Device(d) => d.idle_centi_fe(),
        }
    }
}

/// `ae2:<colour>_<cable>` (e.g. `ae2:fluix_smart_cable`), or a device's `ae2:<name>`.
pub fn parse_block(id: &str) -> Option<Kind> {
    let name = id.strip_prefix("ae2:")?;
    if let Some(d) = Device::ALL.into_iter().find(|d| d.name() == name) {
        return Some(Kind::Device(d));
    }
    let color = COLORS.iter().find(|c| name.strip_prefix(**c).is_some_and(|r| r.starts_with('_')))?;
    let rest = &name[color.len() + 1..];
    Cable::ALL.into_iter().find(|c| c.name() == rest).map(Kind::Cable)
}

pub fn cable_id(color: &str, cable: Cable) -> String {
    format!("ae2:{color}_{}", cable.name())
}

pub fn device_id(d: Device) -> String {
    format!("ae2:{}", d.name())
}

/// Whether two neighbouring AE blocks join: devices join anything; cables join unless both have a
/// colour (not fluix) and the colours differ.
pub fn connects(a: &str, b: &str) -> bool {
    let (Some(ka), Some(kb)) = (parse_block(a), parse_block(b)) else { return false };
    if let (Kind::Cable(_), Kind::Cable(_)) = (ka, kb) {
        let color = |id: &str| COLORS.iter().find(|c| id[4..].starts_with(&format!("{c}_"))).copied();
        return match (color(a), color(b)) {
            (Some("fluix"), _) | (_, Some("fluix")) => true,
            (x, y) => x == y,
        };
    }
    true
}

/// Channels a network offers: 8 on its own, 32 with a controller.
pub fn channel_capacity(has_controller: bool) -> usize {
    if has_controller { 32 } else { 8 }
}

/// The devices that have a channel: the first `capacity` of those that need one, in position order.
pub fn channelled<P: Copy + Ord>(mut users: Vec<P>, capacity: usize) -> Vec<P> {
    users.sort();
    users.truncate(capacity);
    users
}

/// What a bus does each `BUS_PERIOD` ticks: one item.
pub const BUS_PERIOD: i64 = 5;

/// Every block joined to `start` (up to `MAX_NODES`), sorted.
pub fn flood<P: Copy + Ord>(start: P, neighbours: impl Fn(P) -> Vec<P>) -> Vec<P> {
    let mut seen = std::collections::BTreeSet::from([start]);
    let mut queue = std::collections::VecDeque::from([start]);
    while let Some(p) = queue.pop_front() {
        if seen.len() >= MAX_NODES {
            break;
        }
        for n in neighbours(p) {
            if seen.insert(n) {
                queue.push_back(n);
            }
        }
    }
    seen.into_iter().collect()
}

/// The network `start` belongs to, if `start` is its lowest block: the sorted blocks (up to
/// `MAX_NODES`). Gives up as soon as it meets a lower one, so only one block per network pays for the
/// whole scan.
pub fn led_network<P: Copy + Ord>(start: P, neighbours: impl Fn(P) -> Vec<P>) -> Option<Vec<P>> {
    let mut seen = std::collections::BTreeSet::from([start]);
    let mut queue = std::collections::VecDeque::from([start]);
    while let Some(p) = queue.pop_front() {
        if seen.len() >= MAX_NODES {
            break;
        }
        for n in neighbours(p) {
            if n < start {
                return None;
            }
            if seen.insert(n) {
                queue.push_back(n);
            }
        }
    }
    Some(seen.into_iter().collect())
}

/// Power draw of a network for one tick: whole FE to take now, and the hundredths carried over.
pub fn drain(total_centi_fe: i64, carried: i64) -> (i64, i64) {
    let sum = total_centi_fe + carried;
    (sum / 100, sum % 100)
}

/// A network is powered when its cells hold the whole draw of this tick.
pub fn powered(stored_fe: i64, draw_fe: i64) -> bool {
    stored_fe > 0 && stored_fe >= draw_fe
}

/// Which block leads a network: cables and the controller first (a block's own data cannot be read
/// during its own tick, so a cell, acceptor or drive leads only a network without any of them).
pub fn lead_class(kind: Kind) -> u8 {
    match kind {
        Kind::Cable(_) | Kind::Device(Device::Controller) => 0,
        Kind::Device(_) => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        assert_eq!(parse_block("ae2:fluix_smart_cable"), Some(Kind::Cable(Cable::Smart)));
        assert_eq!(parse_block("ae2:light_blue_covered_dense_cable"), Some(Kind::Cable(Cable::CoveredDense)));
        assert_eq!(parse_block("ae2:dense_energy_cell"), Some(Kind::Device(Device::DenseEnergyCell)));
        assert_eq!(parse_block("ae2:controller"), Some(Kind::Device(Device::Controller)));
        assert_eq!(parse_block("ae2:silicon"), None);
        assert_eq!(parse_block("ae2:red_cable"), None);
        assert_eq!(parse_block("mekanism:controller"), None);
        for color in COLORS {
            for c in Cable::ALL {
                assert_eq!(parse_block(&cable_id(color, c)), Some(Kind::Cable(c)));
            }
        }
    }

    #[test]
    fn differently_coloured_cables_stay_apart() {
        assert!(connects("ae2:red_glass_cable", "ae2:red_smart_cable"));
        assert!(!connects("ae2:red_glass_cable", "ae2:blue_glass_cable"));
        assert!(connects("ae2:red_glass_cable", "ae2:fluix_glass_cable"));
        assert!(connects("ae2:blue_glass_cable", "ae2:controller"));
        assert!(!connects("ae2:blue_glass_cable", "minecraft:stone"));
    }

    #[test]
    fn cell_capacities_are_two_fe_per_ae() {
        assert_eq!(Device::EnergyCell.capacity_fe(), 400_000);
        assert_eq!(Device::DenseEnergyCell.capacity_fe(), 3_200_000);
    }

    /// A row of blocks 0..5 with a gap at 3: two networks.
    fn row(p: i32) -> Vec<i32> {
        [p - 1, p + 1].into_iter().filter(|n| (0..6).contains(n) && *n != 3).collect()
    }

    #[test]
    fn only_the_lowest_block_leads_its_network() {
        assert_eq!(led_network(0, row), Some(vec![0, 1, 2]));
        assert_eq!(led_network(1, row), None);
        assert_eq!(led_network(4, row), Some(vec![4, 5]));
        assert_eq!(led_network(5, row), None);
    }

    #[test]
    fn idle_draw_carries_its_hundredths() {
        // a controller (12 FE) and three smart cables (0.2 FE each) = 12.6 FE per tick
        let total = Device::Controller.idle_centi_fe() + 3 * Cable::Smart.idle_centi_fe();
        let (take, rem) = drain(total, 0);
        assert_eq!((take, rem), (12, 60));
        // the carried 0.6 FE joins the next tick: 13.2 FE, 13 now
        assert_eq!(drain(total, rem), (13, 20));
    }

    #[test]
    fn cables_lead_before_devices() {
        assert!(lead_class(Kind::Cable(Cable::Glass)) < lead_class(Kind::Device(Device::EnergyCell)));
        assert_eq!(lead_class(Kind::Device(Device::Controller)), 0);
    }

    #[test]
    fn channels_go_to_the_first_devices_and_a_controller_adds_more() {
        assert_eq!(channel_capacity(false), 8);
        assert_eq!(channel_capacity(true), 32);
        let users: Vec<i32> = (0..10).rev().collect();
        assert_eq!(channelled(users, 8), (0..8).collect::<Vec<_>>());
        assert_eq!(channelled(vec![3, 1], 8), vec![1, 3]);
    }

    #[test]
    fn flood_finds_a_whole_network_from_any_block() {
        assert_eq!(flood(2, row), vec![0, 1, 2]);
        assert_eq!(flood(5, row), vec![4, 5]);
    }

    #[test]
    fn power_needs_the_whole_draw() {
        assert!(powered(12, 12));
        assert!(!powered(0, 0));
        assert!(!powered(11, 12));
    }
}
