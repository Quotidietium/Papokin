use pumpkin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;
use crate::world::World;

use super::PlayerEvent;

/// An event that occurs when a chunk is unloaded on the client of a player.
///
/// This is a pure notification; the chunk data has already been (or is about
/// to be) discarded client-side.
#[derive(Event, Clone)]
pub struct PlayerChunkUnloadEvent {
    /// The player the chunk is unloaded for.
    pub player: Arc<Player>,

    /// The world the chunk is in.
    pub target_world: Arc<World>,

    /// The X coordinate of the chunk.
    pub chunk_x: i32,

    /// The Z coordinate of the chunk.
    pub chunk_z: i32,
}

impl PlayerChunkUnloadEvent {
    /// Creates a new instance of `PlayerChunkUnloadEvent`.
    pub const fn new(
        player: Arc<Player>,
        target_world: Arc<World>,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Self {
        Self {
            player,
            target_world,
            chunk_x,
            chunk_z,
        }
    }
}

impl PlayerEvent for PlayerChunkUnloadEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
