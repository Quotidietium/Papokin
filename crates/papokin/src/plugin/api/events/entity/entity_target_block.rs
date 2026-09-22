use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 实体以方块为目标时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityTargetBlockEvent {
    /// 实体 ID。
    pub entity_id: i32,
    /// 被瞄准的方块位置。
    pub block_pos: BlockPos,
}

impl EntityTargetBlockEvent {
    #[must_use]
    pub const fn new(entity_id: i32, block_pos: BlockPos) -> Self {
        Self {
            entity_id,
            block_pos,
            cancelled: false,
        }
    }
}
