use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a command on a sign is preprocessed before
/// execution.
///
/// Cancelling prevents the command from being executed; `command` may be
/// modified by handlers to rewrite the command before it runs.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerSignCommandPreprocessEvent {
    /// The player running the sign command.
    pub player: Arc<Player>,

    /// The position of the sign block.
    pub block_pos: BlockPos,

    /// The command to be executed (modifiable).
    pub command: String,
}

impl PlayerSignCommandPreprocessEvent {
    /// Creates a new instance of `PlayerSignCommandPreprocessEvent`.
    pub fn new(player: Arc<Player>, block_pos: BlockPos, command: impl Into<String>) -> Self {
        Self {
            player,
            block_pos,
            command: command.into(),
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerSignCommandPreprocessEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
