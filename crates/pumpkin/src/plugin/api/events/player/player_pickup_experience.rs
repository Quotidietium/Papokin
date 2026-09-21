use pumpkin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player picks up an experience orb.
///
/// Cancelling prevents the pickup entirely. `amount` may be modified by
/// handlers and is the amount that mending and experience gain will consume.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerPickupExperienceEvent {
    /// The player picking up the experience.
    pub player: Arc<Player>,

    /// The entity id of the experience orb.
    pub orb_id: i32,

    /// The amount of experience picked up.
    pub amount: i32,
}

impl PlayerPickupExperienceEvent {
    /// Creates a new instance of `PlayerPickupExperienceEvent`.
    pub const fn new(player: Arc<Player>, orb_id: i32, amount: i32) -> Self {
        Self {
            player,
            orb_id,
            amount,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerPickupExperienceEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
