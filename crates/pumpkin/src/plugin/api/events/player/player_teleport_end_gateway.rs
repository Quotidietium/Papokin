use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player teleports through an end gateway.
///
/// Cancelling vetoes the teleport. The host side is not wired yet (the end
/// gateway block is owned by another agent); see the integration notes in
/// the event wiring report.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerTeleportEndGatewayEvent {
    /// The teleporting player.
    pub player: Arc<Player>,

    /// The position of the end gateway block.
    pub gateway: BlockPos,

    /// The position the player teleports from.
    pub from_position: Vector3<f64>,

    /// The position the player teleports to.
    pub to_position: Vector3<f64>,
}

impl PlayerTeleportEndGatewayEvent {
    /// Creates a new instance of `PlayerTeleportEndGatewayEvent`.
    pub const fn new(
        player: Arc<Player>,
        gateway: BlockPos,
        from_position: Vector3<f64>,
        to_position: Vector3<f64>,
    ) -> Self {
        Self {
            player,
            gateway,
            from_position,
            to_position,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerTeleportEndGatewayEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
