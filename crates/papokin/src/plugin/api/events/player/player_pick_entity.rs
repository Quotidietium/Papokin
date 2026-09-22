use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家选取实体（中键点击）并
/// 收到对应的刷怪蛋。
///
/// 取消即否决选取。`result` 物品由宿主计算；
/// 目前来自访客（guest）的修改尚未应用到选取逻辑。
/// 这是一个 Java 协议事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerPickEntityEvent {
    /// 选取实体的玩家。
    pub player: Arc<Player>,

    /// 被拾取实体的实体 ID。
    pub entity_id: i32,

    /// 交付给玩家的结果物品。
    pub result: ItemStack,
}

impl PlayerPickEntityEvent {
    /// 创建 `PlayerPickEntityEvent` 的新实例。
    pub const fn new(player: Arc<Player>, entity_id: i32, result: ItemStack) -> Self {
        Self {
            player,
            entity_id,
            result,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerPickEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
