use papokin_data::item_stack::ItemStack;
use papokin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家的盔甲部件变化时触发的事件。
///
/// 盔甲装备变更传播时触发的纯通知。
/// `new_item` 是正在装备的物品（移除该装备件时为 `None`）。
/// `old_item` 仅在先前内容仍可被观察到时才会填充
/// 位于钩子点；否则可能为 `None`。
#[derive(Event, Clone)]
pub struct PlayerArmorChangeEvent {
    /// 盔甲发生变化的玩家。
    pub player: Arc<Player>,

    /// 发生变化的盔甲槽位（`head`、`chest`、`legs` 或 `feet`）。
    pub slot: String,

    /// 之前装备的物品（如果已知）。
    pub old_item: Option<ItemStack>,

    /// 新装备上的物品，如果有的话。
    pub new_item: Option<ItemStack>,
}

impl PlayerArmorChangeEvent {
    /// 创建 `PlayerArmorChangeEvent` 的新实例。
    pub fn new(
        player: Arc<Player>,
        slot: impl Into<String>,
        old_item: Option<ItemStack>,
        new_item: Option<ItemStack>,
    ) -> Self {
        Self {
            player,
            slot: slot.into(),
            old_item,
            new_item,
        }
    }
}

impl PlayerEvent for PlayerArmorChangeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
