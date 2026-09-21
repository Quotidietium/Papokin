use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

/// An event that occurs when a beacon is activated (its tier goes from 0 to n).
#[cancellable]
#[derive(Event, Clone)]
pub struct BeaconActivatedEvent {
    /// The player attributed with activating the beacon (nearest player in range).
    pub player: Arc<Player>,

    /// Position of the beacon block.
    pub block_pos: BlockPos,
}

impl BeaconActivatedEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, block_pos: BlockPos) -> Self {
        Self {
            player,
            block_pos,
            cancelled: false,
        }
    }
}
