use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 实体对玩家可见时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerShowEntityEvent {
    /// 进行观察的玩家。
    pub player: Arc<Player>,

    /// 被揭示的实体 ID。
    pub entity_id: i32,
}

impl PlayerEvent for PlayerShowEntityEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
