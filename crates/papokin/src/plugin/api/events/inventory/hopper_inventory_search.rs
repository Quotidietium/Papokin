use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 漏斗搜索容器物品栏时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct HopperInventorySearchEvent {
    /// 漏斗的方块位置。
    pub block_pos: BlockPos,

    /// 目标搜索位置。
    pub search_pos: BlockPos,
}

impl HopperInventorySearchEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, search_pos: BlockPos) -> Self {
        Self {
            block_pos,
            search_pos,
            cancelled: false,
        }
    }
}
