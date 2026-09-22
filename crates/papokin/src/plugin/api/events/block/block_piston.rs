use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 活塞推出时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockPistonExtendEvent {
    pub block_pos: BlockPos,
    pub direction: String,
}

impl BlockPistonExtendEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, direction: String) -> Self {
        Self {
            block_pos,
            direction,
            cancelled: false,
        }
    }
}

/// 活塞收回时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockPistonRetractEvent {
    pub block_pos: BlockPos,
    pub direction: String,
}

impl BlockPistonRetractEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, direction: String) -> Self {
        Self {
            block_pos,
            direction,
            cancelled: false,
        }
    }
}
