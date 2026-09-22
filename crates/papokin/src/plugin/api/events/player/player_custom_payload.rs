use bytes::Bytes;
use papokin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家发送自定义负载数据包时发生的事件。
#[derive(Event, Clone)]
pub struct PlayerCustomPayloadEvent {
    /// 发送自定义负载的玩家。
    pub player: Arc<Player>,
    /// 负载通道标识符（例如 `voicechat:request_secret`）。
    pub channel: String,
    /// 原始负载数据。
    pub data: Bytes,
}

impl PlayerCustomPayloadEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, channel: String, data: Bytes) -> Self {
        Self {
            player,
            channel,
            data,
        }
    }
}

impl PlayerEvent for PlayerCustomPayloadEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
