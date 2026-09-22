use std::net::{Ipv4Addr, SocketAddr};

use serde::{Deserialize, Serialize};

/// 服务器查询协议（旧版 Minecraft query）的配置。
///
/// 控制是否启用查询服务以及绑定到哪个地址。
#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct QueryConfig {
    /// 是否启用查询协议。
    pub enabled: bool,
    /// 查询服务绑定的地址和端口。
    pub address: SocketAddr,
}

impl Default for QueryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            address: SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 25565),
        }
    }
}
