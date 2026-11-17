//! `ae2:network_energy`: the energy of a whole network as one energy handler, answered by every AE2 block, so an
//! addon in another crate (a flux accessor that gives the network's energy to FE machines) reaches it with
//! `level.capability(host, &NETWORK_ENERGY, pos, side)` on any block of the network. It is a
//! mod-own key over the game's [`EnergyHandler`]: the amount is what the network's cells hold in FE, inserting
//! fills the cells and extracting drains them (first cell first). It is not `horizon:energy`, so other mods'
//! cables beside an AE2 cable do not drain the network.
use mod_api::{CapabilityKey, Direction, ENERGY, EnergyHandler, LevelRef, ServerHost, Transaction};

use crate::rules::*;
use crate::terminal::net;

pub const NETWORK_ENERGY: CapabilityKey<dyn EnergyHandler> = CapabilityKey::new("ae2:network_energy");

pub struct NetworkEnergy {
    pub level: LevelRef,
    pub pos: [i32; 3],
}

impl NetworkEnergy {
    fn cells(&self, host: &mut dyn ServerHost) -> Vec<[i32; 3]> {
        net(host, self.level, self.pos).cells
    }
}

impl EnergyHandler for NetworkEnergy {
    fn amount(&self, host: &mut dyn ServerHost) -> u32 {
        let mut sum = 0u64;
        for c in self.cells(host) {
            if let Some(cap) = self.level.capability(host, &ENERGY, c, Some(Direction::Up)) {
                sum += cap.amount(host) as u64;
            }
        }
        sum.min(u32::MAX as u64) as u32
    }

    fn capacity(&self, host: &mut dyn ServerHost) -> u32 {
        let mut sum = 0u64;
        for c in self.cells(host) {
            if let Some(cap) = self.level.capability(host, &ENERGY, c, Some(Direction::Up)) {
                sum += cap.capacity(host) as u64;
            }
        }
        sum.min(u32::MAX as u64) as u32
    }

    fn insert(&self, host: &mut dyn ServerHost, tx: &Transaction, amount: u32) -> u32 {
        let mut left = amount;
        for c in self.cells(host) {
            if left == 0 {
                break;
            }
            if let Some(cap) = self.level.capability(host, &ENERGY, c, Some(Direction::Up)) {
                left -= cap.insert(host, tx, left).min(left);
            }
        }
        amount - left
    }

    fn extract(&self, host: &mut dyn ServerHost, tx: &Transaction, amount: u32) -> u32 {
        let mut left = amount;
        for c in self.cells(host) {
            if left == 0 {
                break;
            }
            if let Some(cap) = self.level.capability(host, &ENERGY, c, Some(Direction::Up)) {
                left -= cap.extract(host, tx, left).min(left);
            }
        }
        amount - left
    }
}

/// Whether the kind belongs to the AE2 network (every kind does).
pub fn serves(id: &str) -> bool {
    parse_block(id).is_some()
}
