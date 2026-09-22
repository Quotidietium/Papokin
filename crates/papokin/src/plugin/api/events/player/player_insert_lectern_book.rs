use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家将书放入讲台时发生的事件。
///
/// 取消即否决插入；点击会被消耗，但书仍保持
/// 在玩家手中。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerInsertLecternBookEvent {
    /// 放入书的玩家。
    pub player: Arc<Player>,

    /// 讲台方块的位置。
    pub block_pos: BlockPos,

    /// 正在放入的书。
    pub book: ItemStack,
}

impl PlayerInsertLecternBookEvent {
    /// 创建 `PlayerInsertLecternBookEvent` 的新实例。
    pub const fn new(player: Arc<Player>, block_pos: BlockPos, book: ItemStack) -> Self {
        Self {
            player,
            block_pos,
            book,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerInsertLecternBookEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
