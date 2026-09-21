use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player changes a sign, fired before any
/// validation checks (e.g. the `SignChangeEvent` filtering) are applied.
///
/// Cancelling drops the sign update entirely. `lines` are the raw text lines
/// as sent by the client. This is a Java-protocol event.
#[cancellable]
#[derive(Event, Clone)]
pub struct UncheckedSignChangeEvent {
    /// The player editing the sign.
    pub player: Arc<Player>,

    /// The position of the sign block.
    pub block_pos: BlockPos,

    /// The raw text lines of the sign.
    pub lines: Vec<String>,
}

impl UncheckedSignChangeEvent {
    /// Creates a new instance of `UncheckedSignChangeEvent`.
    #[must_use]
    pub const fn new(player: Arc<Player>, block_pos: BlockPos, lines: Vec<String>) -> Self {
        Self {
            player,
            block_pos,
            lines,
            cancelled: false,
        }
    }
}

impl PlayerEvent for UncheckedSignChangeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
