use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家解开拴绳实体时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerUnleashEntityEvent {
    /// 解开实体拴绳的玩家。
    pub player: Arc<Player>,

    /// 被解除拴绳的实体的 ID。
    pub entity_id: i32,
}

impl PlayerEvent for PlayerUnleashEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
