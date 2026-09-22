use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家停止旁观一个实体时发生的事件（摄像机随之切回）。
/// 会重置为玩家）。
///
/// 取消会阻止摄像机重置；玩家继续旁观。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerStopSpectatingEntityEvent {
    /// 之前处于旁观模式的玩家。
    pub player: Arc<Player>,

    /// 此前被旁观的实体。
    pub target_id: i32,
}

impl PlayerStopSpectatingEntityEvent {
    /// 创建 `PlayerStopSpectatingEntityEvent` 的新实例。
    pub const fn new(player: Arc<Player>, target_id: i32) -> Self {
        Self {
            player,
            target_id,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerStopSpectatingEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
