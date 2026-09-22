use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 服务器相关链接的配置。
///
/// 控制错误报告、支持、社区及其他资源的默认 URL，
/// 同时还允许自定义链接。
#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct ServerLinksConfig {
    /// 是否启用服务器链接。
    pub enabled: bool,
    /// 报告漏洞的 URL。
    pub bug_report: String,
    /// 支持资源的 URL。
    pub support: String,
    /// 服务器状态的 URL。
    pub status: String,
    /// 玩家反馈的 URL。
    pub feedback: String,
    /// 社区页面的 URL。
    pub community: String,
    /// 官方网站的 URL。
    pub website: String,
    /// 论坛的 URL。
    pub forums: String,
    /// 新闻更新的 URL。
    pub news: String,
    /// 公告的 URL。
    pub announcements: String,
    /// 自定义键值链接。
    pub custom: HashMap<String, String>,
}

impl Default for ServerLinksConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bug_report: "https://github.com/Pumpkin-MC/Pumpkin/issues".to_string(),
            support: String::new(),
            status: String::new(),
            feedback: String::new(),
            community: String::new(),
            website: String::new(),
            forums: String::new(),
            news: String::new(),
            announcements: String::new(),
            custom: HashMap::default(),
        }
    }
}
