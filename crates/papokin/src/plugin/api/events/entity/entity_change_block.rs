use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 实体改变世界中的方块时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityChangeBlockEvent {
    /// 实体的 ID。
    pub entity_id: i32,
    /// 方块的位置。
    pub block_pos: BlockPos,
    /// 新的方块状态标识符。
    pub new_block: String,
}

impl EntityChangeBlockEvent {
    #[must_use]
    pub const fn new(entity_id: i32, block_pos: BlockPos, new_block: String) -> Self {
        Self {
            entity_id,
            block_pos,
            new_block,
            cancelled: false,
        }
    }
}
