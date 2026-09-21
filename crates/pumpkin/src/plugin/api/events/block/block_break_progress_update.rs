use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

/// An event that occurs when the break progress of a block is updated for a player.
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockBreakProgressUpdateEvent {
    /// The player breaking the block.
    pub player: Arc<Player>,

    /// Position of the block being broken.
    pub block_pos: BlockPos,

    /// Current break progress from 0.0 to 1.0, or -1.0 when reset.
    pub progress: f32,
}

impl BlockBreakProgressUpdateEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, block_pos: BlockPos, progress: f32) -> Self {
        Self {
            player,
            block_pos,
            progress,
            cancelled: false,
        }
    }
}
