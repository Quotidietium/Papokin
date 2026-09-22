use papokin_data::item_stack::ItemStack;
use papokin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家停止使用物品（如松开
/// 已拉开的弓）。
///
/// 纯通知；在此时点，物品使用停止这一行为本身不可被否决。
#[derive(Event, Clone)]
pub struct PlayerStopUsingItemEvent {
    /// 停止使用物品的玩家。
    pub player: Arc<Player>,

    /// 正在被使用的物品。
    pub item: ItemStack,
}

impl PlayerStopUsingItemEvent {
    /// 创建 `PlayerStopUsingItemEvent` 的新实例。
    pub const fn new(player: Arc<Player>, item: ItemStack) -> Self {
        Self { player, item }
    }
}

impl PlayerEvent for PlayerStopUsingItemEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
