use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

/// 物品被放入堆肥桶时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct CompostItemEvent {
    /// 堆肥该物品的玩家（如果有的话，漏斗也可以堆肥）。
    pub player: Option<Arc<Player>>,

    /// 堆肥桶方块的位置。
    pub block_pos: BlockPos,

    /// 正在堆肥的物品。
    pub item: ItemStack,

    /// 堆肥等级是否会提升。
    pub will_raise_level: bool,
}

impl CompostItemEvent {
    #[must_use]
    pub const fn new(
        player: Option<Arc<Player>>,
        block_pos: BlockPos,
        item: ItemStack,
        will_raise_level: bool,
    ) -> Self {
        Self {
            player,
            block_pos,
            item,
            will_raise_level,
            cancelled: false,
        }
    }
}
