use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddr};

/// RCON（远程控制台）服务的配置。
///
/// 控制是否启用 RCON、连接设置、身份验证和日志记录。
#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct RCONConfig {
    /// 是否启用 RCON。
    pub enabled: bool,
    /// RCON 服务器监听连接的网络地址与端口。
    pub address: SocketAddr,
    /// RCON 身份验证所需的密码。
    pub password: String,
    /// 允许的最大并发 RCON 连接数。
    /// 值为 `0` 表示没有限制。
    pub max_connections: u32,
    /// RCON 事件的日志配置。
    pub logging: RCONLogging,
}

impl Default for RCONConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            address: SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 25575),
            password: String::new(),
            max_connections: 10,
            logging: RCONLogging::default(),
        }
    }
}

/// RCON 的日志设置。
///
/// 控制记录哪些 RCON 事件，包括登录尝试和命令。
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(default)]
pub struct RCONLogging {
    /// 是否记录成功的 RCON 登录。
    pub logged_successfully: bool,
    /// 是否记录密码错误的 RCON 登录尝试。
    pub wrong_password: bool,
    /// 是否记录所有 RCON 命令（无论成败）。
    pub commands: bool,
    /// 是否记录 RCON quit 命令。
    pub quit: bool,
}

impl Default for RCONLogging {
    fn default() -> Self {
        Self {
            logged_successfully: true,
            wrong_password: true,
            commands: true,
            quit: true,
        }
    }
}
