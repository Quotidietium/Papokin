use papokin_data::BlockStateId;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::{entity::player::Player, world::World};

/// 方块被施肥时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockFertilizeEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub player: Option<Arc<Player>>,
    pub changed_blocks: Vec<(BlockPos, BlockStateId)>,
}

impl BlockFertilizeEvent {
    #[must_use]
    pub const fn new(
        block_pos: BlockPos,
        world: Arc<World>,
        player: Option<Arc<Player>>,
        changed_blocks: Vec<(BlockPos, BlockStateId)>,
    ) -> Self {
        Self {
            block_pos,
            world,
            player,
            changed_blocks,
            cancelled: false,
        }
    }
}
