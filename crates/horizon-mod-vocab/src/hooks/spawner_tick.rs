//! The `spawner_tick` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// On an authoritative server, the mob spawner block at `pos` in `dimension` is about to
    /// run its server tick (`BaseSpawner.serverTick`), before the game checks whether a player
    /// is in range (`player_near`: an alive player within the spawner's required range) or
    /// whether the `spawner_blocks_work` rule lets it run. `powered` is whether a neighbour gives
    /// the block redstone power (`Level.hasNeighborSignal`), for a spawner a redstone signal
    /// should stop, as Apothic Spawners' does. Set `cancel` and this tick does nothing, not even
    /// counting its spawn delay down. Spawner minecarts and trial spawners do not fire it.
    #[hook(
        side = "server",
        contract = "cancel",
        out = "cancel",
        neoforge = "none (NeoForge has no such event; Apothic Spawners mixes into BaseSpawner.serverTick)",
        cancellable = true
    )]
    fn spawner_tick();
}
