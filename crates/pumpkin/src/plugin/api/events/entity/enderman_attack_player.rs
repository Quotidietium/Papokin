use std::sync::Arc;

use pumpkin_macros::{Event, cancellable};

use crate::entity::player::Player;

/// An event that occurs when an enderman attacks a player.
#[cancellable]
#[derive(Event, Clone)]
pub struct EndermanAttackPlayerEvent {
    /// The ID of the attacking enderman.
    pub entity_id: i32,

    /// The player being attacked.
    pub player: Arc<Player>,
}

impl EndermanAttackPlayerEvent {
    #[must_use]
    pub const fn new(entity_id: i32, player: Arc<Player>) -> Self {
        Self {
            entity_id,
            player,
            cancelled: false,
        }
    }
}
