use papokin_data::item_stack::ItemStack;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家将物品与盔甲槽位交换时发生的事件。
///
/// 在玩家自己的物品栏界面点击盔甲槽位时触发
/// 光标上持有可装备物品，或对某个物品使用快捷栏交换键时
/// 盔甲槽位。取消此事件将否决这次交换。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerSwapWithEquipmentSlotEvent {
    /// 交换物品的玩家。
    pub player: Arc<Player>,

    /// 与其交换的装备槽位（例如 `head`、`chest`、`legs`、
    /// `feet`).
    pub slot: String,

    /// 当前装备在槽位中的物品，如果有的话。
    pub equipped_item: Option<ItemStack>,

    /// 正在装备的物品（来自光标或快捷栏），如果有的话。
    pub cursor_item: Option<ItemStack>,
}

impl PlayerSwapWithEquipmentSlotEvent {
    /// 创建 `PlayerSwapWithEquipmentSlotEvent` 的新实例。
    pub fn new(
        player: Arc<Player>,
        slot: impl Into<String>,
        equipped_item: Option<ItemStack>,
        cursor_item: Option<ItemStack>,
    ) -> Self {
        Self {
            player,
            slot: slot.into(),
            equipped_item,
            cursor_item,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerSwapWithEquipmentSlotEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
