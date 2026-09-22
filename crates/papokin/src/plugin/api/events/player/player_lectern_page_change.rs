use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家更改讲台上书本的页面时发生的事件。
///
/// 取消即否决翻页；`new_page` 可以被处理器修改
/// 以重定向页面。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerLecternPageChangeEvent {
    /// 翻页的玩家。
    pub player: Arc<Player>,

    /// 讲台方块的位置。
    pub block_pos: BlockPos,

    /// 讲台上的书。
    pub book: ItemStack,

    /// 新的页码。
    pub new_page: i32,
}

impl PlayerLecternPageChangeEvent {
    /// 创建 `PlayerLecternPageChangeEvent` 的新实例。
    pub const fn new(
        player: Arc<Player>,
        block_pos: BlockPos,
        book: ItemStack,
        new_page: i32,
    ) -> Self {
        Self {
            player,
            block_pos,
            book,
            new_page,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerLecternPageChangeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
