use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use super::PlayerEvent;
use crate::entity::player::Player;

/// 玩家更改/注册插件消息通道时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerChannelEvent {
    /// 相关的玩家。
    pub player: Arc<Player>,

    /// 插件消息通道的名称。
    pub channel: String,
}

impl PlayerEvent for PlayerChannelEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
