use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 熔炉开始烧炼物品时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct FurnaceStartSmeltEvent {
    /// 熔炉方块的位置。
    pub block_pos: BlockPos,

    /// 待熔炼物品的注册表键。
    pub source_item: String,

    /// 所需的总烹饪时间（以刻计）。
    pub cooking_time: u32,
}

impl FurnaceStartSmeltEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, source_item: String, cooking_time: u32) -> Self {
        Self {
            block_pos,
            source_item,
            cooking_time,
            cancelled: false,
        }
    }
}
