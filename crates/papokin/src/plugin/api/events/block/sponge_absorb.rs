use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 海绵吸水时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct SpongeAbsorbEvent {
    pub block_pos: BlockPos,
}

impl SpongeAbsorbEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos) -> Self {
        Self {
            block_pos,
            cancelled: false,
        }
    }
}
