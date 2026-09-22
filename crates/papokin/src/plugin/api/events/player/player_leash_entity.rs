use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家拴住实体时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerLeashEntityEvent {
    /// 给实体拴绳的玩家。
    pub player: Arc<Player>,

    /// 被拴实体的 ID。
    pub entity_id: i32,

    /// 拴持实体或栅栏的 ID。
    pub holder_id: i32,
}

impl PlayerEvent for PlayerLeashEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
