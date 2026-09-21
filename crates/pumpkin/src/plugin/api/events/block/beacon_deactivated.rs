use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

/// An event that occurs when a beacon is deactivated (its tier drops to 0).
#[cancellable]
#[derive(Event, Clone)]
pub struct BeaconDeactivatedEvent {
    /// The player attributed with deactivating the beacon, if any
    /// (nearest player in range when the transition was observed).
    pub player: Option<Arc<Player>>,

    /// Position of the beacon block.
    pub block_pos: BlockPos,
}

impl BeaconDeactivatedEvent {
    #[must_use]
    pub const fn new(player: Option<Arc<Player>>, block_pos: BlockPos) -> Self {
        Self {
            player,
            block_pos,
            cancelled: false,
        }
    }
}
