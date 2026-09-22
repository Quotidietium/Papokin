use papokin_data::Block;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 方块消退或融化时发生的事件（例如冰、雪）。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockFadeEvent {
    /// 正在消退的方块的位置。
    pub block_pos: BlockPos,

    /// 正在消退的方块。
    pub block: &'static Block,
}

impl BlockFadeEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, block: &'static Block) -> Self {
        Self {
            block_pos,
            block,
            cancelled: false,
        }
    }
}
