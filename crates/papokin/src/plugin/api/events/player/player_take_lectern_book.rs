use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家从讲台取书时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerTakeLecternBookEvent {
    /// 取走书的玩家。
    pub player: Arc<Player>,

    /// 讲台的位置。
    pub block_pos: BlockPos,

    /// 书的物品堆。
    pub book: ItemStack,
}

impl PlayerEvent for PlayerTakeLecternBookEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
