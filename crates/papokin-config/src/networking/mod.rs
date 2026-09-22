use proxy::ProxyConfig;
use query::QueryConfig;
use rcon::RCONConfig;
use serde::{Deserialize, Serialize};

use crate::LANBroadcastConfig;
use java::JavaConfig;

/// 身份验证配置。
pub mod auth;
/// 数据包压缩配置。
pub mod compression;
/// Java 协议网络配置。
pub mod java;
/// LAN 广播发现配置。
pub mod lan_broadcast;
/// 反向代理与 BungeeCord/Velocity 配置。
pub mod proxy;
/// GS4 Query 协议配置。
pub mod query;
/// RCON 远程控制台配置。
pub mod rcon;

/// 数据包限制器配置。
pub mod packet_limiter;
pub use packet_limiter::PacketLimiterConfig;

/// 服务器网络功能的配置。
///
/// 涵盖身份验证、查询、RCON、代理、数据包压缩、
/// 以及 LAN 广播行为。
#[derive(Deserialize, Serialize, Default)]
#[serde(default)]
pub struct NetworkingConfig {
    /// 针对服务器状态请求的查询协议设置。
    pub query: QueryConfig,
    /// RCON（远程控制台）配置。
    pub rcon: RCONConfig,
    /// 代理相关网络设置。
    pub proxy: ProxyConfig,
    /// LAN 广播设置。
    pub lan_broadcast: LANBroadcastConfig,
    /// Java 版配置设置。
    pub java: JavaConfig,
}
