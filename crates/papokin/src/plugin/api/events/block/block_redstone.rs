use std::sync::Arc;

use papokin_data::Block;
use papokin_data::BlockStateId;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

use crate::world::World;

use super::BlockEvent;

/// 方块的红石等级变化时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockRedstoneEvent {
    /// 红石信号强度发生变化的世界。
    pub world: Arc<World>,

    /// 红石信号发生变化的方块状态 ID。
    pub block_state_id: BlockStateId,

    /// 方块的位置。
    pub block_pos: BlockPos,

    /// 原红石电流。
    pub old_current: i32,

    /// 新的红石电流。
    pub new_current: i32,
}

impl BlockRedstoneEvent {
    /// 创建新的 `BlockRedstoneEvent`。
    #[must_use]
    pub const fn new(
        world: Arc<World>,
        block_state_id: BlockStateId,
        block_pos: BlockPos,
        old_current: i32,
        new_current: i32,
    ) -> Self {
        Self {
            world,
            block_state_id,
            block_pos,
            old_current,
            new_current,
            cancelled: false,
        }
    }
}

impl BlockEvent for BlockRedstoneEvent {
    fn get_block(&self) -> &Block {
        Block::from_state_id(self.block_state_id)
    }
}
