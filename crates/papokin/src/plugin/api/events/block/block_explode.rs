use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 方块爆炸时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockExplodeEvent {
    pub block_pos: BlockPos,
    pub yield_rate: f32,
}

impl BlockExplodeEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, yield_rate: f32) -> Self {
        Self {
            block_pos,
            yield_rate,
            cancelled: false,
        }
    }
}
