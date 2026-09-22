use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 实体进入方块时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityEnterBlockEvent {
    /// 实体的 ID。
    pub entity_id: i32,
    /// 进入的方块的位置。
    pub block_pos: BlockPos,
}

impl EntityEnterBlockEvent {
    #[must_use]
    pub const fn new(entity_id: i32, block_pos: BlockPos) -> Self {
        Self {
            entity_id,
            block_pos,
            cancelled: false,
        }
    }
}
