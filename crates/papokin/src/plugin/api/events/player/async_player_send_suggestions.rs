use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 命令建议发送给
/// 玩家。
///
/// 取消会抑制建议项；`suggestions` 可以被
/// 处理程序来添加或移除条目。
#[cancellable]
#[derive(Event, Clone)]
pub struct AsyncPlayerSendSuggestionsEvent {
    /// 接收补全建议的玩家。
    pub player: Arc<Player>,

    /// 当前的命令缓冲区。
    pub buffer: String,

    /// 要发送的补全建议（可修改）。
    pub suggestions: Vec<String>,
}

impl AsyncPlayerSendSuggestionsEvent {
    /// 创建新的 `AsyncPlayerSendSuggestionsEvent` 实例。
    pub fn new(player: Arc<Player>, buffer: impl Into<String>, suggestions: Vec<String>) -> Self {
        Self {
            player,
            buffer: buffer.into(),
            suggestions,
            cancelled: false,
        }
    }
}

impl PlayerEvent for AsyncPlayerSendSuggestionsEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
