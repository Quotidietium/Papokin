use crate::entity::player::Player;
use papokin_data::Block;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

/// 方块被点燃时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockIgniteEvent {
    /// 被点燃方块的位置。
    pub block_pos: BlockPos,

    /// 点燃此方块的方块。
    pub igniting_block: &'static Block,

    /// 点燃方块的玩家（如果有的话）。
    pub player: Option<Arc<Player>>,
}

impl BlockIgniteEvent {
    #[must_use]
    pub const fn new(
        block_pos: BlockPos,
        igniting_block: &'static Block,
        player: Option<Arc<Player>>,
    ) -> Self {
        Self {
            block_pos,
            igniting_block,
            player,
            cancelled: false,
        }
    }
}
