use serde::{Deserialize, Serialize};

/// 代理支持的配置。
///
/// 允许与 Velocity 和 `BungeeCord` 等代理服务器集成。
#[derive(Deserialize, Serialize, Default)]
#[serde(default)]
pub struct ProxyConfig {
    /// 是否启用代理支持。
    pub enabled: bool,
    /// Velocity 代理集成的配置。
    pub velocity: VelocityConfig,
    /// `BungeeCord` 代理集成的配置。
    pub bungeecord: BungeeCordConfig,
    /// 采用 Ed25519 认证的 Vine 现代代理集成配置。
    pub vine: VineConfig,
}

/// `BungeeCord` 代理集成的配置。
#[derive(Deserialize, Serialize, Default)]
#[serde(default)]
pub struct BungeeCordConfig {
    /// 是否启用 `BungeeCord` 支持。
    pub enabled: bool,
    /// 用于验证来自 `BungeeCord` 代理连接的共享密钥
    /// 代理，由 `BungeeGuard` 插件提供。设置后，转发的
    /// profile 属性必须包含一个 `bungeeguard-token` 属性，其内容为
    /// 此密钥，否则连接将被拒绝。这也会阻止
    /// 直接连接而非经代理连接的玩家。
    pub secret: String,
}

/// Velocity 代理集成的配置。
#[derive(Deserialize, Serialize, Default)]
#[serde(default)]
pub struct VelocityConfig {
    /// 是否启用 Velocity 支持。
    pub enabled: bool,
    /// 用于验证来自 Velocity 代理连接的共享密钥。
    pub secret: String,
}

/// 采用 Ed25519 认证与重放防护的 Vine 代理集成配置。
#[derive(Deserialize, Serialize, Default, Clone)]
#[serde(default)]
pub struct VineConfig {
    /// 是否启用 Vine 支持。
    pub enabled: bool,
    /// Vine 代理的 Ed25519 公钥（64 个十六进制字符）。
    /// 后端只需此公钥即可验证被转发的玩家身份。
    pub public_key: String,
    /// 可选的共享密钥字符串。如果提供了它且 `public_key` 为空，
    /// 公钥会自动从该密钥派生。
    pub secret: String,
}
