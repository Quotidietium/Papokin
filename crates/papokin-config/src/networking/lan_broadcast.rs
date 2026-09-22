use serde::{Deserialize, Serialize};

/// 服务器局域网广播的配置。
///
/// 控制服务器是否可在局域网中被发现，以及可选的 MOTD 和端口设置。
#[derive(Deserialize, Serialize, Default)]
#[serde(default)]
pub struct LANBroadcastConfig {
    /// 是否启用局域网广播。
    pub enabled: bool,
    /// 面向 LAN 客户端的可选单行每日消息（MOTD）。
    /// 默认为移除换行符后的服务器 MOTD。
    pub motd: Option<String>,
    /// 用于 LAN 广播的可选端口。
    /// 在 Docker 容器等环境中可获得可预测的端口。
    pub port: Option<u16>,
}
