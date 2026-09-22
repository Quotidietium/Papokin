use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 发射器未能发射物品时发生的事件
/// (例如发射器为空)。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockFailedDispenseEvent {
    /// 发射器方块的位置。
    pub block_pos: BlockPos,

    /// 未能发射的物品。当发射器未持有物品时为空。
    /// 没有可发射的物品。
    pub item: ItemStack,
}

impl BlockFailedDispenseEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, item: ItemStack) -> Self {
        Self {
            block_pos,
            item,
            cancelled: false,
        }
    }
}
