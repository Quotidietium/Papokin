use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::vector3::Vector3;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player jumps.
///
/// The client does not send a jump destination, so `to_position` mirrors
/// `from_position` (matching Paper). Cancelling skips the server-side effects
/// of the jump (jump statistic and exhaustion); the physical motion is
/// client-authoritative and cannot be rolled back here.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerJumpEvent {
    /// The jumping player.
    pub player: Arc<Player>,

    /// The position the player jumps from.
    pub from_position: Vector3<f64>,

    /// The position the player jumps to.
    pub to_position: Vector3<f64>,
}

impl PlayerJumpEvent {
    /// Creates a new instance of `PlayerJumpEvent`.
    pub const fn new(
        player: Arc<Player>,
        from_position: Vector3<f64>,
        to_position: Vector3<f64>,
    ) -> Self {
        Self {
            player,
            from_position,
            to_position,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerJumpEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
