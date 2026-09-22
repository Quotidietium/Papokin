use std::sync::Arc;

use crate::entity::player::Player;
use papokin_data::screen::WindowType;
use papokin_macros::Event;

use super::PlayerEvent;

/// 玩家关闭物品栏时触发的事件。
#[derive(Event, Clone)]
pub struct InventoryCloseEvent {
    /// 关闭物品栏的玩家。
    pub player: Arc<Player>,

    /// 被关闭的物品栏的窗口类型。
    pub window_type: Option<WindowType>,
}

impl InventoryCloseEvent {
    /// 创建 `InventoryCloseEvent` 的新实例。
    ///
    /// # Arguments
    ///
    /// - `player`：指向触发该事件的玩家的引用计数指针。
    /// - `window_type`：物品栏的窗口类型。
    ///
    /// # Returns
    ///
    /// 一个包含指定数据的新 `InventoryCloseEvent` 实例。
    pub fn new(player: &Arc<Player>, window_type: Option<WindowType>) -> Self {
        Self {
            player: Arc::clone(player),
            window_type,
        }
    }
}

impl PlayerEvent for InventoryCloseEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
