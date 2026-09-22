use papokin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// 玩家的客户端完成一个刻（tick）时发生的事件。
///
/// 这是每个客户端刻触发一次的高频纯通知
/// 每个玩家一个数据包；分发器内的零监听器提前返回
/// 在未注册插件时保持低开销。这是 Java 协议的
/// 事件。
#[derive(Event, Clone)]
pub struct ClientTickEndEvent {
    /// 客户端刻已结束的玩家。
    pub player: Arc<Player>,
}

impl ClientTickEndEvent {
    /// 创建新的 `ClientTickEndEvent` 实例。
    pub const fn new(player: Arc<Player>) -> Self {
        Self { player }
    }
}

impl PlayerEvent for ClientTickEndEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
