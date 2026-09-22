use serde::{Deserialize, Serialize};

/// 面向 Java 客户端的服务器资源包配置。
#[derive(Deserialize, Serialize, Default)]
#[serde(default)]
pub struct ResourcePackConfig {
    /// Java 版客户端资源包配置。
    pub java: JavaResourcePackConfig,
}

/// Java 专用的资源包配置（单一 URL/哈希）
#[derive(Deserialize, Serialize, Default)]
#[serde(default)]
pub struct JavaResourcePackConfig {
    /// 是否启用资源包系统。
    pub enabled: bool,
    /// 资源包的 URL。
    pub url: String,
    /// 资源包的 SHA1 哈希（40 个字符）。
    pub sha1: String,
    /// 显示给玩家的自定义提示文本组件；留空则不显示。
    pub prompt_message: String,
    /// 是否强制玩家接受资源包。
    pub force: bool,
}
