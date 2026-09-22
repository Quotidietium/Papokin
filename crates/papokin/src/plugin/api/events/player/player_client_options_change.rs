use papokin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家的客户端选项在游戏过程中发生变化时发生的事件
/// (即游戏阶段的 client-information 数据包)。
///
/// 仅携带新值的纯通知。配置阶段
/// (登录前) 的 client information 无法触发此事件，因为
/// [`Player`] 尚不存在。
#[derive(Event, Clone)]
pub struct PlayerClientOptionsChangeEvent {
    /// 客户端选项发生变化的玩家。
    pub player: Arc<Player>,

    /// 客户端的新区域设置。
    pub locale: String,

    /// 以区块为单位的新视距。
    pub view_distance: i32,

    /// 新的聊天可见性设置（`enabled`、`commands`、`hidden`）。
    pub chat_visibility: String,

    /// 是否启用聊天颜色。
    pub chat_colors: bool,

    /// 新的主手设置（`left` 或 `right`）。
    pub main_hand: String,

    /// 新的可见皮肤部位位掩码。
    pub skin_parts: u32,
}

impl PlayerClientOptionsChangeEvent {
    /// 创建 `PlayerClientOptionsChangeEvent` 的新实例。
    pub const fn new(
        player: Arc<Player>,
        locale: String,
        view_distance: i32,
        chat_visibility: String,
        chat_colors: bool,
        main_hand: String,
        skin_parts: u32,
    ) -> Self {
        Self {
            player,
            locale,
            view_distance,
            chat_visibility,
            chat_colors,
            main_hand,
            skin_parts,
        }
    }
}

impl PlayerEvent for PlayerClientOptionsChangeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
