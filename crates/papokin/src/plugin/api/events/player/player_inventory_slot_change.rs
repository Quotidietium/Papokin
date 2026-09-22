use papokin_data::item_stack::ItemStack;
use papokin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家物品栏槽位中的物品变化时触发的事件。
///
/// 这是由屏幕处理器触发的高频通知事件
/// 槽位追踪器。槽位索引是当前
/// 被跟踪的屏幕处理器（玩家物品栏槽位跟随容器槽位）。
/// `old_item` 反映该槽位最后一次同步到客户端的状态，
/// 当没有已知的先前状态时返回 `None`。
#[derive(Event, Clone)]
pub struct PlayerInventorySlotChangeEvent {
    /// 物品栏发生变化的玩家。
    pub player: Arc<Player>,

    /// 发生变化的物品栏槽位。
    pub slot: i32,

    /// 之前位于槽位中的物品，如果已知的话。
    pub old_item: Option<ItemStack>,

    /// 现在位于槽位中的物品，如果有的话。
    pub new_item: Option<ItemStack>,
}

impl PlayerInventorySlotChangeEvent {
    /// 创建 `PlayerInventorySlotChangeEvent` 的新实例。
    pub const fn new(
        player: Arc<Player>,
        slot: i32,
        old_item: Option<ItemStack>,
        new_item: Option<ItemStack>,
    ) -> Self {
        Self {
            player,
            slot,
            old_item,
            new_item,
        }
    }
}

impl PlayerEvent for PlayerInventorySlotChangeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
