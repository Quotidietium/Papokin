use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 物品在物品栏之间移动时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct InventoryMoveItemEvent {
    /// 来源物品栏的位置。
    pub source_pos: BlockPos,

    /// 目标物品栏位置。
    pub target_pos: BlockPos,

    /// 被移动物品的注册表键。
    pub item_id: String,

    /// 移动的物品数量。
    pub item_amount: u32,
}

impl InventoryMoveItemEvent {
    #[must_use]
    pub const fn new(
        source_pos: BlockPos,
        target_pos: BlockPos,
        item_id: String,
        item_amount: u32,
    ) -> Self {
        Self {
            source_pos,
            target_pos,
            item_id,
            item_amount,
            cancelled: false,
        }
    }
}
