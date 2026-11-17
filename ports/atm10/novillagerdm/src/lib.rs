//! No Villager Death Messages for Horizon.
//!
//! The original removes the line vanilla's `Villager.die` writes to the server log for every
//! villager that dies (`Villager ... died, message: '...'`), which floods the console of servers
//! with villager farms. Written from that public description, never from the mod's code.
//!
//! Horizon's server does not write that line at all (no `Villager.die` log is ported), so the
//! behaviour the mod exists for already holds and this port has nothing to change. It stays a mod
//! so that a modpack that lists it resolves, and so that the day Horizon logs the line, the
//! suppression lands here (see README.md, "Needs from Horizon").
use mod_api::Hooks;

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {}
