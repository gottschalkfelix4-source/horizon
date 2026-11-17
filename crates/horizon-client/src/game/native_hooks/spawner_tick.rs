use super::*;

/// The hook points. Each does nothing in Horizon.
impl Game<'_> {
    /// On an authoritative server, the mob spawner block at `pos` in `dimension` is about to
    /// run its server tick (`BaseSpawner.serverTick`), before the game checks whether a player
    /// is in range (`player_near`: an alive player within the spawner's required range) or
    /// whether the `spawner_blocks_work` rule lets it run. `powered` is whether a neighbour gives
    /// the block redstone power (`Level.hasNeighborSignal`), for a spawner a redstone signal
    /// should stop, as Apothic Spawners' does. Set `cancel` and this tick does nothing, not even
    /// counting its spawn delay down. Spawner minecarts and trial spawners do not fire it.
    pub(in crate::game) fn native_spawner_tick(
        &mut self,
        dimension: &str,
        pos: [i32; 3],
        player_near: bool,
        powered: bool,
        cancel: &mut bool,
    ) {
        self.fire_spawner_tick(dimension, pos, player_near, powered, cancel);
    }
}
