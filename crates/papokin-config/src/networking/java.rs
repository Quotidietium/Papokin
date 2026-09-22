use crate::{AuthenticationConfig, CompressionConfig, PacketLimiterConfig};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::num::NonZero;

/// Java 版客户端连接的配置。
#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct JavaConfig {
    /// 是否接受 Java 版客户端。
    pub enabled: bool,
    /// Java 版服务器将要绑定的地址和端口。
    pub address: SocketAddr,
    /// 是否启用数据包加密。启用在线模式时必需。
    pub encryption: bool,
    /// 是否启用在线模式。需要有效的 Minecraft 账户。
    pub online_mode: bool,
    /// 服务器允许的最大玩家数。指定 `0` 可禁用该限制。
    pub max_players: u32,
    /// 玩家的最大视距。
    pub view_distance: NonZero<u8>,
    /// 最大模拟视距。
    pub simulation_distance: NonZero<u8>,
    /// 向 Java 客户端发送保活数据包的时间间隔（秒）。
    #[serde(
        alias = "keep-alive-time",
        alias = "keep_alive_interval",
        alias = "keep-alive-interval"
    )]
    pub keep_alive_time: u64,
    /// Java 版数据包压缩设置。
    pub compression: CompressionConfig,
    /// 每日消息（MOTD）；显示在状态界面上的服务器描述。
    pub motd: String,
    /// 客户端连接的身份验证设置。
    pub authentication: AuthenticationConfig,
    /// 数据包速率限制设置。
    pub packet_limiter: PacketLimiterConfig,
}

impl Default for JavaConfig {
    fn default() -> Self {
        let address = "0.0.0.0:25565"
            .parse()
            .unwrap_or_else(|_| std::net::SocketAddr::from(([0, 0, 0, 0], 25565)));
        let view_distance = NonZero::new(16).unwrap_or(NonZero::<u8>::MIN);
        let simulation_distance = NonZero::new(10).unwrap_or(NonZero::<u8>::MIN);
        Self {
            enabled: true,
            address,
            encryption: true,
            online_mode: true,
            max_players: 1000,
            view_distance,
            simulation_distance,
            keep_alive_time: 15,
            compression: CompressionConfig::default(),
            motd: "A blazingly fast Pumpkin server!".to_string(),
            authentication: AuthenticationConfig::default(),
            packet_limiter: PacketLimiterConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_keep_alive_time() {
        let config = JavaConfig::default();
        assert_eq!(config.keep_alive_time, 15);
    }

    #[test]
    fn keep_alive_time_deserialization() {
        let toml_snake = r"
            keep_alive_time = 20
        ";
        let config: JavaConfig = toml::from_str(toml_snake).unwrap();
        assert_eq!(config.keep_alive_time, 20);

        let toml_kebab = r"
            keep-alive-time = 25
        ";
        let config: JavaConfig = toml::from_str(toml_kebab).unwrap();
        assert_eq!(config.keep_alive_time, 25);

        let toml_interval = r"
            keep_alive_interval = 30
        ";
        let config: JavaConfig = toml::from_str(toml_interval).unwrap();
        assert_eq!(config.keep_alive_time, 30);

        let toml_interval_kebab = r"
            keep-alive-interval = 35
        ";
        let config: JavaConfig = toml::from_str(toml_interval_kebab).unwrap();
        assert_eq!(config.keep_alive_time, 35);
    }
}
