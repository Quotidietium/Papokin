use papokin_data::Block;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

use super::BlockEvent;

/// 方块被放置时发生的事件。
///
/// 此事件包含关于放置方块的玩家、被放置的方块等信息，
/// 被放置时依托的方块，以及玩家能否建造。
#[cancellable]
#[derive(Event, Clone)]
pub struct BlockPlaceEvent {
    /// 放置方块的玩家。
    pub player: Arc<Player>,

    /// 正在被放置的方块。
    pub block_placed: &'static Block,

    /// 新方块放置时所贴靠的方块。
    pub block_placed_against: &'static Block,

    /// 方块被放置的位置。
    pub block_position: BlockPos,

    /// 一个布尔值，表示玩家是否可以建造。
    pub can_build: bool,
}

impl BlockPlaceEvent {
    #[must_use]
    pub const fn new(
        player: Arc<Player>,
        block_placed: &'static Block,
        block_placed_against: &'static Block,
        block_position: BlockPos,
        can_build: bool,
    ) -> Self {
        Self {
            player,
            block_placed,
            block_placed_against,
            block_position,
            can_build,
            cancelled: false,
        }
    }
}

impl BlockEvent for BlockPlaceEvent {
    fn get_block(&self) -> &Block {
        self.block_placed
    }
}
