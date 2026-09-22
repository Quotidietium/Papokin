use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家执行命令时发生的事件
///
/// 如果事件被取消，命令将不会执行。
///
/// 此事件包含关于玩家和正在执行的命令的信息。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerCommandSendEvent {
    /// 正在执行命令的玩家。
    pub player: Arc<Player>,

    /// 正在执行的命令
    pub command: String,
}

impl PlayerCommandSendEvent {
    /// 创建 `PlayerCommandSendEvent` 的新实例。
    ///
    /// # Arguments
    /// - `player`：正在执行命令的玩家的引用。
    /// - `command`：正在执行的命令。
    ///
    /// # Returns
    /// 一个新的 `PlayerCommandSendEvent` 实例。
    pub const fn new(player: Arc<Player>, command: String) -> Self {
        Self {
            player,
            command,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerCommandSendEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
