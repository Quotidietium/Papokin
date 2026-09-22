use papokin_macros::{Event, cancellable};
use papokin_util::text::TextComponent;
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家发送聊天消息时触发的异步事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct AsyncPlayerChatEvent {
    /// 发送消息的玩家。
    pub player: Arc<Player>,

    /// 聊天消息内容。
    pub message: String,

    /// 格式化后的聊天消息。
    pub format: TextComponent,
}

impl PlayerEvent for AsyncPlayerChatEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
