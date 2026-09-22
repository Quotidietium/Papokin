use crate::entity::player::Player;
use papokin_data::Block;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

/// 玩家损坏/破坏方块时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockDamageEvent {
    /// 损坏方块的玩家。
    pub player: Arc<Player>,

    /// 正在受损的方块。
    pub block: &'static Block,

    /// 方块的位置。
    pub block_pos: BlockPos,

    /// 方块是否被立即破坏。
    pub insta_break: bool,
}

impl BlockDamageEvent {
    #[must_use]
    pub const fn new(
        player: Arc<Player>,
        block: &'static Block,
        block_pos: BlockPos,
        insta_break: bool,
    ) -> Self {
        Self {
            player,
            block,
            block_pos,
            insta_break,
            cancelled: false,
        }
    }
}
