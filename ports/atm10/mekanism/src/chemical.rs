//! Mekanism's chemical capability (gases, infuse types, pigments, slurries): `mekanism:chemicals`,
//! a mod-own capability over the game's generic [`ResourceHandler`], so pipes and machines of
//! other ports reach a chemical tank the way they reach items and fluids. The tank is one saved
//! compound `{Chemical: "mekanism:hydrogen", Amount: n}` under the block entity's `chemical_<name>` key.
use mod_api::{
    BlockSide, CapabilityKey, Compound, ResourceHandler, ServerHost, Tag, Transaction,
};

/// One chemical: its registry id (`mekanism:hydrogen`).
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
pub struct ChemicalResource(pub String);

/// The capability: the chemical tanks of a block, as one side reaches them.
pub const CHEMICALS: CapabilityKey<dyn ResourceHandler<ChemicalResource>> =
    CapabilityKey::new("mekanism:chemicals");

/// What a tank holds, as saved.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Tank {
    pub chemical: Option<String>,
    pub amount: u32,
}

/// The key a named tank is saved under in the block entity's data.
fn key_of(name: &str) -> String {
    format!("chemical_{name}")
}

impl Tank {
    pub fn load(nbt: &Compound, name: &str) -> Tank {
        let Some(Tag::Compound(t)) = nbt.get(&key_of(name).as_str().into()) else {
            return Tank::default();
        };
        let chemical = match t.get(&"Chemical".into()) {
            Some(Tag::String(s)) => Some(s.to_string_lossy()).filter(|c| !c.is_empty()),
            _ => None,
        };
        let amount = match t.get(&"Amount".into()) {
            Some(Tag::Int(v)) => (*v).max(0) as u32,
            _ => 0,
        };
        if chemical.is_none() || amount == 0 { Tank::default() } else { Tank { chemical, amount } }
    }

    pub fn save(&self, nbt: &mut Compound, name: &str) {
        let mut t = Compound::new();
        t.insert("Chemical".into(), Tag::String(self.chemical.clone().unwrap_or_default().as_str().into()));
        t.insert("Amount".into(), Tag::Int(self.amount.min(i32::MAX as u32) as i32));
        nbt.insert(key_of(name).as_str().into(), Tag::Compound(t));
    }

    /// Fills with up to `amount` of `chemical` into a tank of `capacity`: how much went in.
    pub fn fill(&mut self, chemical: &str, amount: u32, capacity: u32) -> u32 {
        if self.amount > 0 && self.chemical.as_deref() != Some(chemical) {
            return 0;
        }
        let moved = amount.min(capacity.saturating_sub(self.amount));
        if moved > 0 {
            self.chemical = Some(chemical.to_owned());
            self.amount += moved;
        }
        moved
    }

    /// Drains up to `amount` of `chemical`: how much came out.
    pub fn drain(&mut self, chemical: &str, amount: u32) -> u32 {
        if self.chemical.as_deref() != Some(chemical) {
            return 0;
        }
        let moved = amount.min(self.amount);
        self.amount -= moved;
        if self.amount == 0 {
            self.chemical = None;
        }
        moved
    }
}

/// One tank of a block: its name, size, what may go in and come out, and the one chemical it takes.
#[derive(Clone, Copy, Debug)]
pub struct TankSpec {
    pub name: &'static str,
    pub capacity: u32,
    pub insert: bool,
    pub extract: bool,
    pub only: Option<&'static str>,
}

/// The chemical tanks of the block entity at `at`, as `specs` say: index `i` is `specs[i]`.
pub struct BlockTanks {
    pub at: BlockSide,
    pub specs: &'static [TankSpec],
}

impl BlockTanks {
    fn read(&self, host: &mut dyn ServerHost, i: usize) -> Tank {
        let Some(entity) = host.block_entity(self.at.level, self.at.pos) else {
            return Tank::default();
        };
        entity.data(&*host).map(|d| Tank::load(&d, self.specs[i].name)).unwrap_or_default()
    }

    fn write(&self, host: &mut dyn ServerHost, i: usize, tank: &Tank) {
        let Some(entity) = host.block_entity(self.at.level, self.at.pos) else { return };
        let mut patch = Compound::new();
        tank.save(&mut patch, self.specs[i].name);
        let _ = entity.merge_data(host, patch);
    }
}

impl ResourceHandler<ChemicalResource> for BlockTanks {
    fn size(&self, _: &mut dyn ServerHost) -> usize {
        self.specs.len()
    }
    fn resource(&self, host: &mut dyn ServerHost, i: usize) -> Option<ChemicalResource> {
        self.read(host, i).chemical.map(ChemicalResource)
    }
    fn amount(&self, host: &mut dyn ServerHost, i: usize) -> u32 {
        self.read(host, i).amount
    }
    fn capacity(&self, _: &mut dyn ServerHost, i: usize, r: &ChemicalResource) -> u32 {
        let spec = &self.specs[i];
        if spec.only.is_some_and(|o| o != r.0) { 0 } else { spec.capacity }
    }
    fn insert(
        &self,
        host: &mut dyn ServerHost,
        _tx: &Transaction,
        i: usize,
        resource: &ChemicalResource,
        amount: u32,
    ) -> u32 {
        let spec = &self.specs[i];
        if !spec.insert || spec.only.is_some_and(|o| o != resource.0) {
            return 0;
        }
        let mut tank = self.read(host, i);
        let moved = tank.fill(&resource.0, amount, spec.capacity);
        if moved > 0 {
            self.write(host, i, &tank);
        }
        moved
    }
    fn extract(
        &self,
        host: &mut dyn ServerHost,
        _tx: &Transaction,
        i: usize,
        resource: &ChemicalResource,
        amount: u32,
    ) -> u32 {
        if !self.specs[i].extract {
            return 0;
        }
        let mut tank = self.read(host, i);
        let moved = tank.drain(&resource.0, amount);
        if moved > 0 {
            self.write(host, i, &tank);
        }
        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tank_takes_one_chemical_at_a_time_up_to_its_capacity() {
        let mut t = Tank::default();
        assert_eq!(t.fill("mekanism:hydrogen", 600, 1000), 600);
        assert_eq!(t.fill("mekanism:oxygen", 100, 1000), 0);
        assert_eq!(t.fill("mekanism:hydrogen", 600, 1000), 400);
        assert_eq!(t.drain("mekanism:oxygen", 10), 0);
        assert_eq!(t.drain("mekanism:hydrogen", 1500), 1000);
        assert_eq!(t, Tank::default());
    }

    #[test]
    fn a_tank_survives_saving() {
        let mut t = Tank::default();
        t.fill("mekanism:ethene", 250, 1000);
        let mut nbt = Compound::new();
        t.save(&mut nbt, "in");
        assert_eq!(Tank::load(&nbt, "in"), t);
        assert_eq!(Tank::load(&nbt, "out"), Tank::default());
    }
}
