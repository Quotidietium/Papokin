use papokin_macros::{Event, cancellable};
use papokin_util::text::TextComponent;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家加入游戏前发生的事件。
///
/// 如果事件被取消，玩家将被踢出服务器。
///
/// 此事件包含关于玩家加入的信息，并可选择设置踢出消息。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerLoginEvent {
    /// 正在加入游戏的玩家。
    pub player: Arc<Player>,

    /// 事件被取消时显示的踢出消息。
    pub kick_message: TextComponent,
}

impl PlayerLoginEvent {
    /// 创建 `PlayerLoginEvent` 的新实例。
    ///
    /// # Arguments
    /// - `player`：正在加入游戏的玩家的引用。
    /// - `kick_message`：加入时显示的消息。
    ///
    /// # Returns
    /// 一个新的 `PlayerLoginEvent` 实例。
    pub const fn new(player: Arc<Player>, kick_message: TextComponent) -> Self {
        Self {
            player,
            kick_message,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerLoginEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
