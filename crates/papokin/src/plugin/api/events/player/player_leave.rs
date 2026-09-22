use papokin_macros::{Event, cancellable};
use papokin_util::text::TextComponent;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家离开游戏时发生的事件。
///
/// 此事件包含关于玩家离开的信息，以及离开时显示的消息。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerLeaveEvent {
    /// 正在离开游戏的玩家。
    pub player: Arc<Player>,

    /// 玩家离开时显示的消息。
    pub leave_message: TextComponent,
}

impl PlayerLeaveEvent {
    /// 创建 `PlayerLeaveEvent` 的新实例。
    ///
    /// # Arguments
    /// - `player`：正在离开游戏的玩家的引用。
    /// - `leave_message`：离开时显示的消息。
    ///
    /// # Returns
    /// 一个新的 `PlayerLeaveEvent` 实例。
    pub const fn new(player: Arc<Player>, leave_message: TextComponent) -> Self {
        Self {
            player,
            leave_message,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerLeaveEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
