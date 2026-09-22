use papokin_data::Block;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;

/// 方块（如流体）从一处扩散/流动到另一处时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockFromToEvent {
    /// 来源方块的位置。
    pub from_pos: BlockPos,

    /// 目标方块位置。
    pub to_pos: BlockPos,

    /// 正在流动的方块。
    pub block: &'static Block,
}

impl BlockFromToEvent {
    #[must_use]
    pub const fn new(from_pos: BlockPos, to_pos: BlockPos, block: &'static Block) -> Self {
        Self {
            from_pos,
            to_pos,
            block,
            cancelled: false,
        }
    }
}
