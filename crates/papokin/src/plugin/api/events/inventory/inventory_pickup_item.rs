use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 容器（如漏斗）拾取物品实体时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct InventoryPickupItemEvent {
    /// 容器的方块位置。
    pub block_pos: BlockPos,

    /// 被拾取物品的实体 ID。
    pub item_entity_id: i32,

    /// 物品的注册表键。
    pub item_id: String,
}

impl InventoryPickupItemEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, item_entity_id: i32, item_id: String) -> Self {
        Self {
            block_pos,
            item_entity_id,
            item_id,
            cancelled: false,
        }
    }
}
