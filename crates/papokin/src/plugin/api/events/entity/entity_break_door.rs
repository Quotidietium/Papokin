use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 实体破坏门时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityBreakDoorEvent {
    /// 实体的 ID。
    pub entity_id: i32,
    /// 门方块的位置。
    pub block_pos: BlockPos,
}

impl EntityBreakDoorEvent {
    #[must_use]
    pub const fn new(entity_id: i32, block_pos: BlockPos) -> Self {
        Self {
            entity_id,
            block_pos,
            cancelled: false,
        }
    }
}
