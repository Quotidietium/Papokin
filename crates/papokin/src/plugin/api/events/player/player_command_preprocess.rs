use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家命令执行前触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerCommandPreprocessEvent {
    /// 发送命令的玩家。
    pub player: Arc<Player>,

    /// 原始命令字符串。
    pub command: String,
}

impl PlayerEvent for PlayerCommandPreprocessEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
