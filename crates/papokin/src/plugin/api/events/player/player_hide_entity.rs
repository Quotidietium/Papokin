use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 实体对玩家隐藏时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerHideEntityEvent {
    /// 对其隐藏该实体的玩家。
    pub player: Arc<Player>,

    /// 被隐藏实体的 ID。
    pub entity_id: i32,
}

impl PlayerEvent for PlayerHideEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
