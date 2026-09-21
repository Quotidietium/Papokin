use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player stops spectating an entity (the camera
/// is reset to the player).
///
/// Cancelling prevents the camera reset; the player keeps spectating.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerStopSpectatingEntityEvent {
    /// The player that was spectating.
    pub player: Arc<Player>,

    /// The entity that was being spectated.
    pub target_id: i32,
}

impl PlayerStopSpectatingEntityEvent {
    /// Creates a new instance of `PlayerStopSpectatingEntityEvent`.
    pub const fn new(player: Arc<Player>, target_id: i32) -> Self {
        Self {
            player,
            target_id,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerStopSpectatingEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
