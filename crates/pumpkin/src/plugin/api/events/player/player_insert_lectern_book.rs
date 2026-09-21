use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player inserts a book into a lectern.
///
/// Cancelling vetoes the insertion; the click is consumed but the book stays
/// in the player's hand.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerInsertLecternBookEvent {
    /// The player inserting the book.
    pub player: Arc<Player>,

    /// The position of the lectern block.
    pub block_pos: BlockPos,

    /// The book being inserted.
    pub book: ItemStack,
}

impl PlayerInsertLecternBookEvent {
    /// Creates a new instance of `PlayerInsertLecternBookEvent`.
    pub const fn new(player: Arc<Player>, block_pos: BlockPos, book: ItemStack) -> Self {
        Self {
            player,
            block_pos,
            book,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerInsertLecternBookEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
