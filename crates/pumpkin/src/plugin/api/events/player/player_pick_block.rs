use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player picks a block (middle click / pick
/// block) and receives the corresponding item.
///
/// Cancelling vetoes the pick. The `result` item is computed by the host;
/// modifications from the guest are currently not applied to the pick logic.
/// This is a Java-protocol event.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerPickBlockEvent {
    /// The player picking the block.
    pub player: Arc<Player>,

    /// The position of the picked block.
    pub block_pos: BlockPos,

    /// The resulting item given to the player.
    pub result: ItemStack,
}

impl PlayerPickBlockEvent {
    /// Creates a new instance of `PlayerPickBlockEvent`.
    pub const fn new(player: Arc<Player>, block_pos: BlockPos, result: ItemStack) -> Self {
        Self {
            player,
            block_pos,
            result,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerPickBlockEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
