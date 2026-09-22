use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家开始旁观一个实体时发生的事件。
///
/// 取消会阻止将摄像机设为目标实体。此
/// 是 Java 协议事件（旁观数据包）。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerStartSpectatingEntityEvent {
    /// 正在观战的玩家。
    pub player: Arc<Player>,

    /// 正被旁观的实体。
    pub target_id: i32,
}

impl PlayerStartSpectatingEntityEvent {
    /// 创建 `PlayerStartSpectatingEntityEvent` 的新实例。
    pub const fn new(player: Arc<Player>, target_id: i32) -> Self {
        Self {
            player,
            target_id,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerStartSpectatingEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
