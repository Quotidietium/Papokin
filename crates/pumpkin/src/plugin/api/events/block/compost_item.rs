use pumpkin_data::item_stack::ItemStack;
use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

/// An event that occurs when an item is inserted into a composter.
#[cancellable]
#[derive(Event, Clone)]
pub struct CompostItemEvent {
    /// The player composting the item, if any (hoppers can compost too).
    pub player: Option<Arc<Player>>,

    /// Position of the composter block.
    pub block_pos: BlockPos,

    /// The item being composted.
    pub item: ItemStack,

    /// Whether the compost level will be raised.
    pub will_raise_level: bool,
}

impl CompostItemEvent {
    #[must_use]
    pub const fn new(
        player: Option<Arc<Player>>,
        block_pos: BlockPos,
        item: ItemStack,
        will_raise_level: bool,
    ) -> Self {
        Self {
            player,
            block_pos,
            item,
            will_raise_level,
            cancelled: false,
        }
    }
}
