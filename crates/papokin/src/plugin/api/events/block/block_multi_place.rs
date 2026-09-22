use papokin_data::BlockStateId;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::{entity::player::Player, world::World};

/// 一次放置多个方块时发生的事件（如床、门）。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockMultiPlaceEvent {
    pub player: Arc<Player>,
    pub world: Arc<World>,
    pub placed_blocks: Vec<(BlockPos, BlockStateId)>,
}

impl BlockMultiPlaceEvent {
    #[must_use]
    pub const fn new(
        player: Arc<Player>,
        world: Arc<World>,
        placed_blocks: Vec<(BlockPos, BlockStateId)>,
    ) -> Self {
        Self {
            player,
            world,
            placed_blocks,
            cancelled: false,
        }
    }
}
