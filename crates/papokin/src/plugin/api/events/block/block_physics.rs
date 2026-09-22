use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 执行方块物理检查时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockPhysicsEvent {
    pub block_pos: BlockPos,
    pub changed_pos: BlockPos,
}

impl BlockPhysicsEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, changed_pos: BlockPos) -> Self {
        Self {
            block_pos,
            changed_pos,
            cancelled: false,
        }
    }
}
