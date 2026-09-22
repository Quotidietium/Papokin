use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家选取方块（中键 / 选取方块键）时发生的事件。
/// 方块）并获得对应的物品。
///
/// 取消即否决选取。`result` 物品由宿主计算；
/// 目前来自访客（guest）的修改尚未应用到选取逻辑。
/// 这是一个 Java 协议事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerPickBlockEvent {
    /// 选取方块的玩家。
    pub player: Arc<Player>,

    /// 被选取方块的位置。
    pub block_pos: BlockPos,

    /// 交付给玩家的结果物品。
    pub result: ItemStack,
}

impl PlayerPickBlockEvent {
    /// 创建 `PlayerPickBlockEvent` 的新实例。
    pub const fn new(player: Arc<Player>, block_pos: BlockPos, result: ItemStack) -> Self {
        Self {
            player,
            block_pos,
            result,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerPickBlockEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
