use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家发送聊天消息时触发的事件。
///
/// 此事件包含关于发送者、消息和接收者的信息。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerChatEvent {
    /// 发送消息的玩家。
    pub player: Arc<Player>,

    /// 正在发送的消息。
    pub message: String,

    /// 消息的接收者。如果为空，则消息会广播给所有玩家。
    pub recipients: Vec<Arc<Player>>,

    /// 消息可选的 256 字节加密签名。
    pub signature: Option<Vec<u8>>,
}

impl PlayerChatEvent {
    /// 创建 `PlayerChatEvent` 的新实例。
    ///
    /// # Arguments
    /// - `player`：正在发送消息的玩家的引用。
    /// - `message`：正在发送的消息。
    /// - `recipients`：消息的接收者。若为空，则消息广播给所有玩家。
    /// - `signature`：消息的可选加密签名。
    ///
    /// # Returns
    /// 一个新的 `PlayerChatEvent` 实例。
    pub const fn new(
        player: Arc<Player>,
        message: String,
        recipients: Vec<Arc<Player>>,
        signature: Option<Vec<u8>>,
    ) -> Self {
        Self {
            player,
            message,
            recipients,
            signature,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerChatEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
