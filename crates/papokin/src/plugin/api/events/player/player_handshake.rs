use papokin_macros::{Event, cancellable};

/// 握手的意图。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HandshakeIntention {
    /// 客户端请求服务器状态（服务器列表 ping）。
    Status,

    /// 客户端想要登录。
    Login,
}

/// 客户端发送握手数据包时发生的事件。
///
/// 取消会立即断开客户端连接，不再做后续处理。
///
/// 该连接尚无玩家对象，因此此事件不会
/// 实现 `PlayerEvent`。这是 Java 协议事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerHandshakeEvent {
    /// 客户端的 IP 地址。
    pub ip_address: String,

    /// 客户端连接时使用的主机名。
    pub hostname: String,

    /// 客户端的协议版本。
    pub protocol_version: i32,

    /// 握手的意图（状态或登录）。
    pub intention: HandshakeIntention,
}

impl PlayerHandshakeEvent {
    /// 创建 `PlayerHandshakeEvent` 的新实例。
    pub fn new(
        ip_address: impl Into<String>,
        hostname: impl Into<String>,
        protocol_version: i32,
        intention: HandshakeIntention,
    ) -> Self {
        Self {
            ip_address: ip_address.into(),
            hostname: hostname.into(),
            protocol_version,
            intention,
            cancelled: false,
        }
    }
}
