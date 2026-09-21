use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player changes the page of a book on a lectern.
///
/// Cancelling vetoes the page change; `new_page` may be modified by handlers
/// to redirect the page.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerLecternPageChangeEvent {
    /// The player changing the page.
    pub player: Arc<Player>,

    /// The position of the lectern block.
    pub block_pos: BlockPos,

    /// The book on the lectern.
    pub book: ItemStack,

    /// The new page number.
    pub new_page: i32,
}

impl PlayerLecternPageChangeEvent {
    /// Creates a new instance of `PlayerLecternPageChangeEvent`.
    pub const fn new(
        player: Arc<Player>,
        block_pos: BlockPos,
        book: ItemStack,
        new_page: i32,
    ) -> Self {
        Self {
            player,
            block_pos,
            book,
            new_page,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerLecternPageChangeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
