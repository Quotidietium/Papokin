use super::PlayerEvent;
use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 向玩家发送服务器链接时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerLinksSendEvent {
    /// 该玩家。
    pub player: Arc<Player>,
    /// 已发送的链接。
    pub links: Vec<String>,
}

impl PlayerLinksSendEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, links: Vec<String>) -> Self {
        Self {
            player,
            links,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerLinksSendEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
