use papokin_macros::{Event, cancellable};
use papokin_util::text::TextComponent;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家生成并加入游戏时发生的事件。
///
/// 此事件包含关于玩家加入的信息，以及加入时显示的消息。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerJoinEvent {
    /// 正在加入游戏的玩家。
    pub player: Arc<Player>,

    /// 玩家加入时显示的消息。
    pub join_message: TextComponent,
}

impl PlayerJoinEvent {
    /// 创建 `PlayerJoinEvent` 的新实例。
    ///
    /// # Arguments
    /// - `player`：正在加入游戏的玩家的引用。
    /// - `join_message`：加入时显示的消息。
    ///
    /// # Returns
    /// 一个新的 `PlayerJoinEvent` 实例。
    pub const fn new(player: Arc<Player>, join_message: TextComponent) -> Self {
        Self {
            player,
            join_message,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerJoinEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
