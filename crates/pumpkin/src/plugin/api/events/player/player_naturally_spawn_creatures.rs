use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when creatures are about to be naturally spawned
/// around a player.
///
/// Cancelling suppresses the natural spawn batch for that chunk. The host
/// side is not wired yet (the natural spawner is owned by another agent);
/// see the integration notes in the event wiring report.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerNaturallySpawnCreaturesEvent {
    /// The player the creatures spawn around.
    pub player: Arc<Player>,

    /// The X coordinate of the spawning chunk.
    pub chunk_x: i32,

    /// The Z coordinate of the spawning chunk.
    pub chunk_z: i32,
}

impl PlayerNaturallySpawnCreaturesEvent {
    /// Creates a new instance of `PlayerNaturallySpawnCreaturesEvent`.
    pub const fn new(player: Arc<Player>, chunk_x: i32, chunk_z: i32) -> Self {
        Self {
            player,
            chunk_x,
            chunk_z,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerNaturallySpawnCreaturesEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
