use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家用弓搭箭（开始蓄力）时发生的事件。
/// 拉弓）。
///
/// 取消会阻止拉弓开始。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerReadyArrowEvent {
    /// 搭箭待发的玩家。
    pub player: Arc<Player>,

    /// 正在使用的弓。
    pub bow: ItemStack,

    /// 正在被搭上弓的箭（将会被消耗的投射物）。
    pub arrow: ItemStack,
}

impl PlayerReadyArrowEvent {
    /// 创建 `PlayerReadyArrowEvent` 的新实例。
    pub const fn new(player: Arc<Player>, bow: ItemStack, arrow: ItemStack) -> Self {
        Self {
            player,
            bow,
            arrow,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerReadyArrowEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
