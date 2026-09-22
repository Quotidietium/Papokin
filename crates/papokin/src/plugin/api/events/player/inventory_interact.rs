use std::sync::Arc;

use crate::entity::player::Player;
use papokin_data::{item_stack::ItemStack, screen::WindowType};
use papokin_inventory::screen_handler::ClickType;
use papokin_macros::{Event, cancellable};

use super::PlayerEvent;

/// 玩家在物品栏中点击时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct InventoryClickEvent {
    /// 执行交互的玩家。
    pub player: Arc<Player>,

    /// 正在与之交互的物品栏的窗口类型。
    pub window_type: Option<WindowType>,

    /// 所执行点击的类型。
    pub click_type: ClickType,

    /// 被点击的槽位索引。
    pub slot: i16,

    /// 被点击的原始槽位编号，可直接传给 #getItem(int)
    /// 此槽位编号在该视图内是唯一的。
    pub raw_slot: i16,

    /// 被点击槽位中原有的物品堆。
    pub clicked_item: Option<ItemStack>,

    /// 光标处当前的 `ItemStack`。
    pub cursor: Option<ItemStack>,

    /// 如果 `ClickType` 为 `NUMBER_KEY`，此字段将返回所按键的索引（0-8）。
    pub hotbar_button: i32,
}

impl InventoryClickEvent {
    /// 创建 `InventoryClickEvent` 的新实例。
    ///
    /// # Arguments
    ///
    /// - `player`：指向触发该事件的玩家的引用计数指针。
    /// - `window_type`：物品栏的窗口类型。
    /// - `click_type`：点击类型。
    /// - `slot`：被点击的槽位索引。
    /// - `raw_slot`：原始槽位索引。
    /// - `clicked_item`：被点击槽位中原有的物品堆。
    /// - `cursor`：光标上的物品堆。
    /// - `hotbar_button`：按下的快捷栏按键（0-8）。
    ///
    /// # Returns
    ///
    /// 一个包含指定数据的新 `InventoryClickEvent` 实例。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        player: &Arc<Player>,
        window_type: Option<WindowType>,
        click_type: ClickType,
        slot: i16,
        raw_slot: i16,
        clicked_item: Option<ItemStack>,
        cursor: Option<ItemStack>,
        hotbar_button: i32,
    ) -> Self {
        Self {
            player: Arc::clone(player),
            window_type,
            click_type,
            slot,
            raw_slot,
            clicked_item,
            cursor,
            hotbar_button,
            cancelled: false,
        }
    }
}

impl PlayerEvent for InventoryClickEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
