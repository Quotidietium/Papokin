use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 实体放置方块时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityPlaceEvent {
    /// 实体的 ID。
    pub entity_id: i32,
    /// 被放置方块的位置。
    pub block_pos: BlockPos,
    /// 所放置方块的状态标识符。
    pub block_name: String,
}

impl EntityPlaceEvent {
    #[must_use]
    pub const fn new(entity_id: i32, block_pos: BlockPos, block_name: String) -> Self {
        Self {
            entity_id,
            block_pos,
            block_name,
            cancelled: false,
        }
    }
}
