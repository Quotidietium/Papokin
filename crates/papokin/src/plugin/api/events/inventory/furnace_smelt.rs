use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 熔炉烧炼物品时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct FurnaceSmeltEvent {
    /// 熔炉方块的位置。
    pub block_pos: BlockPos,

    /// 正在熔炼的来源物品。
    pub source_item: String,

    /// 熔炼得到的结果物品。
    pub result_item: String,
}

impl FurnaceSmeltEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, source_item: String, result_item: String) -> Self {
        Self {
            block_pos,
            source_item,
            result_item,
            cancelled: false,
        }
    }
}
