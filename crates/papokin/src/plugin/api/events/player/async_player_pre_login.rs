use papokin_macros::{Event, cancellable};
use papokin_util::text::TextComponent;
use std::net::SocketAddr;
use uuid::Uuid;

/// 连接尝试预登录时发生的异步事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct AsyncPlayerPreLoginEvent {
    /// 玩家的用户名。
    pub player_name: String,

    /// 玩家的唯一 ID。
    pub player_uuid: Uuid,

    /// 远程 IP 地址。
    pub ip_address: SocketAddr,

    /// 连接被拒绝时的踢出消息。
    pub kick_message: TextComponent,
}
