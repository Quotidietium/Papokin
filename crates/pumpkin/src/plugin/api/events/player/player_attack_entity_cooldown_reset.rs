use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player's attack cooldown is reset by attacking
/// an entity.
///
/// Cancelling keeps the previous cooldown progress, so the attack is treated
/// as if the cooldown had not reset.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerAttackEntityCooldownResetEvent {
    /// The attacking player.
    pub player: Arc<Player>,

    /// The entity id of the entity being attacked.
    pub target_id: i32,
}

impl PlayerAttackEntityCooldownResetEvent {
    /// Creates a new instance of `PlayerAttackEntityCooldownResetEvent`.
    pub const fn new(player: Arc<Player>, target_id: i32) -> Self {
        Self {
            player,
            target_id,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerAttackEntityCooldownResetEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
