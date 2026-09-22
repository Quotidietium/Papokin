use papokin_macros::Event;
use uuid::Uuid;

/// 玩家连接关闭时触发的事件。
///
/// 这是一个纯通知，会在每次连接拆除时触发，
/// 包括从未完成登录的连接；因此它
/// 不携带玩家对象，也不实现 `PlayerEvent`。
#[derive(Event, Clone)]
pub struct PlayerConnectionCloseEvent {
    /// 正在断开连接的玩家的 UUID。
    pub player_uuid: Uuid,

    /// 断开连接的玩家名称（当连接从未
    /// 到达登录阶段）。
    pub player_name: String,

    /// 正在断开连接的玩家的 IP 地址。
    pub ip_address: String,
}

impl PlayerConnectionCloseEvent {
    /// 创建 `PlayerConnectionCloseEvent` 的新实例。
    pub fn new(
        player_uuid: Uuid,
        player_name: impl Into<String>,
        ip_address: impl Into<String>,
    ) -> Self {
        Self {
            player_uuid,
            player_name: player_name.into(),
            ip_address: ip_address.into(),
        }
    }
}
