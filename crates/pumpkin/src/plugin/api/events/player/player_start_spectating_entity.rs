use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player starts spectating an entity.
///
/// Cancelling prevents the camera from being set to the target entity. This
/// is a Java-protocol event (spectate packet).
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerStartSpectatingEntityEvent {
    /// The spectating player.
    pub player: Arc<Player>,

    /// The entity being spectated.
    pub target_id: i32,
}

impl PlayerStartSpectatingEntityEvent {
    /// Creates a new instance of `PlayerStartSpectatingEntityEvent`.
    pub const fn new(player: Arc<Player>, target_id: i32) -> Self {
        Self {
            player,
            target_id,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerStartSpectatingEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
