use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 方块发射物品时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockDispenseEvent {
    pub block_pos: BlockPos,
    pub item_name: String,
}

impl BlockDispenseEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, item_name: String) -> Self {
        Self {
            block_pos,
            item_name,
            cancelled: false,
        }
    }
}
