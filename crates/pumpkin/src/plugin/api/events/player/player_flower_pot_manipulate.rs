use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player manipulates the content of a flower pot
/// (planting a flower or taking it out).
///
/// Cancelling vetoes the manipulation.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerFlowerPotManipulateEvent {
    /// The player manipulating the flower pot.
    pub player: Arc<Player>,

    /// The position of the flower pot block.
    pub block_pos: BlockPos,

    /// The item being put into or taken from the pot.
    pub item: ItemStack,
}

impl PlayerFlowerPotManipulateEvent {
    /// Creates a new instance of `PlayerFlowerPotManipulateEvent`.
    pub const fn new(player: Arc<Player>, block_pos: BlockPos, item: ItemStack) -> Self {
        Self {
            player,
            block_pos,
            item,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerFlowerPotManipulateEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
