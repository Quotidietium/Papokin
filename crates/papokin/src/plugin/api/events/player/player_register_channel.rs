use super::PlayerEvent;
use crate::entity::player::Player;
use papokin_macros::{Event, cancellable};
use std::sync::Arc;

/// 为玩家注册插件通道时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerRegisterChannelEvent {
    /// 该玩家。
    pub player: Arc<Player>,
    /// 通道名称。
    pub channel: String,
}

impl PlayerRegisterChannelEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, channel: String) -> Self {
        Self {
            player,
            channel,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerRegisterChannelEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
