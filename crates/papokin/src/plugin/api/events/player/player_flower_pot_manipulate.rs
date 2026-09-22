use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家操作花盆中的内容时发生的事件
/// (种下花朵或将其取出)。
///
/// 取消即否决该操作。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerFlowerPotManipulateEvent {
    /// 操作花盆的玩家。
    pub player: Arc<Player>,

    /// 花盆方块的位置。
    pub block_pos: BlockPos,

    /// 放入锅中或从锅中取出的物品。
    pub item: ItemStack,
}

impl PlayerFlowerPotManipulateEvent {
    /// 创建 `PlayerFlowerPotManipulateEvent` 的新实例。
    pub const fn new(player: Arc<Player>, block_pos: BlockPos, item: ItemStack) -> Self {
        Self {
            player,
            block_pos,
            item,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerFlowerPotManipulateEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
