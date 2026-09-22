use super::PlayerEvent;
use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 玩家响应资源包请求时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerResourcePackStatusEvent {
    /// 该玩家。
    pub player: Arc<Player>,
    /// 资源包 ID。
    pub pack_id: String,
    /// 状态描述。
    pub status: String,
}

impl PlayerResourcePackStatusEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, pack_id: String, status: String) -> Self {
        Self {
            player,
            pack_id,
            status,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerResourcePackStatusEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
