use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs before a player attacks an entity, allowing the attack
/// to be cancelled before any damage is computed.
#[cancellable]
#[derive(Event, Clone)]
pub struct PrePlayerAttackEntityEvent {
    /// The attacking player.
    pub player: Arc<Player>,

    /// The entity id of the entity about to be attacked.
    pub target_id: i32,
}

impl PrePlayerAttackEntityEvent {
    /// Creates a new instance of `PrePlayerAttackEntityEvent`.
    pub const fn new(player: Arc<Player>, target_id: i32) -> Self {
        Self {
            player,
            target_id,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PrePlayerAttackEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
