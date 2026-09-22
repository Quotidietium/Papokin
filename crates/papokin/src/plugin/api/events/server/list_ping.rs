use std::{
    fmt,
    net::{SocketAddr, ToSocketAddrs},
};

use papokin_macros::Event;
use papokin_util::text::TextComponent;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerListPingAddress {
    host: String,
    port: u16,
}

impl ServerListPingAddress {
    #[must_use]
    pub const fn new(host: String, port: u16) -> Self {
        Self { host, port }
    }

    #[must_use]
    pub fn from_socket_addr(address: SocketAddr) -> Self {
        Self {
            host: address.ip().to_string(),
            port: address.port(),
        }
    }

    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    #[must_use]
    pub fn as_socket_addr(&self) -> Option<SocketAddr> {
        (self.host.as_str(), self.port)
            .to_socket_addrs()
            .ok()
            .and_then(|mut addrs| addrs.next())
    }
}

impl fmt::Display for ServerListPingAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.host.contains(':') {
            write!(f, "[{}]:{}", self.host, self.port)
        } else {
            write!(f, "{}:{}", self.host, self.port)
        }
    }
}

/// 服务器响应状态 ping 时发生的事件。
#[derive(Event, Clone)]
pub struct ServerListPingEvent {
    /// 客户端 ping 服务器时使用的主机名。
    pub(crate) hostname: String,

    /// ping 请求的来源地址。
    pub(crate) address: ServerListPingAddress,

    /// 服务器列表中显示的 MOTD。
    pub motd: TextComponent,

    /// 最大玩家数。
    pub max_players: u32,

    /// 当前在线玩家数量。
    pub num_players: u32,

    /// 以 data URI 表示的服务器图标（如有）。
    pub favicon: Option<String>,
}

impl ServerListPingEvent {
    /// 创建新的 `ServerListPingEvent`。
    #[must_use]
    pub fn new(
        hostname: String,
        address: SocketAddr,
        motd: TextComponent,
        max_players: u32,
        num_players: u32,
        favicon: Option<String>,
    ) -> Self {
        Self {
            hostname,
            address: ServerListPingAddress::from_socket_addr(address),
            motd,
            max_players,
            num_players,
            favicon,
        }
    }

    /// 客户端在状态握手期间提供的主机名。
    #[must_use]
    pub fn hostname(&self) -> &str {
        &self.hostname
    }

    /// 请求状态 ping 的客户端的远程套接字地址。
    #[must_use]
    pub const fn address(&self) -> &ServerListPingAddress {
        &self.address
    }
}
