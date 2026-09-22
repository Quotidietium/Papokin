use papokin_data::Block;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 方块自然形成时发生的事件（例如冰、雪、圆石）。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockFormEvent {
    /// 方块形成的位置。
    pub block_pos: BlockPos,

    /// 成形后的方块类型。
    pub block: &'static Block,
}

impl BlockFormEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, block: &'static Block) -> Self {
        Self {
            block_pos,
            block,
            cancelled: false,
        }
    }
}
