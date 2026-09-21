use pumpkin_macros::Event;
use pumpkin_util::math::vector3::Vector3;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs after a player has respawned.
///
/// Pure notification fired once the respawn completed; observe-only
/// counterpart of the earlier
/// [`super::player_respawn::PlayerRespawnEvent`].
#[derive(Event, Clone)]
pub struct PlayerPostRespawnEvent {
    /// The respawned player.
    pub player: Arc<Player>,

    /// The location the player respawned at.
    pub respawn_location: Vector3<f64>,

    /// Whether the respawn location is the player's bed/respawn-anchor spawn.
    pub is_bed_spawn: bool,
}

impl PlayerPostRespawnEvent {
    /// Creates a new instance of `PlayerPostRespawnEvent`.
    pub const fn new(
        player: Arc<Player>,
        respawn_location: Vector3<f64>,
        is_bed_spawn: bool,
    ) -> Self {
        Self {
            player,
            respawn_location,
            is_bed_spawn,
        }
    }
}

impl PlayerEvent for PlayerPostRespawnEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
