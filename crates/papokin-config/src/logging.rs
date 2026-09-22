use serde::{Deserialize, Serialize};

/// 服务器日志行为的配置。
///
/// 控制日志输出、格式和文件设置。
#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// 是否启用日志记录。
    pub enabled: bool,
    /// 控制台与文件输出的最低日志级别（"trace"、"debug"、"info"、"warn"、"error"、"off"）。
    pub level: String,
    /// 是否在日志输出中包含线程名称。
    pub threads: bool,
    /// 是否在日志输出中包含线程 ID。
    pub thread_ids: bool,
    /// 是否在日志输出中包含 target（模块/组件路径）。
    pub target: bool,
    /// 是否启用彩色日志输出。
    pub color: bool,
    /// 日志条目中是否包含时间戳。
    pub timestamp: bool,
    /// 时间戳的格式描述（使用 `time` 的格式描述语法）。
    pub timestamp_format: String,
    /// 日志文件的路径。
    pub file: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level: "info".to_string(),
            threads: false,
            thread_ids: false,
            target: false,
            color: true,
            timestamp: true,
            timestamp_format: "[hour]:[minute]:[second]".to_string(),
            file: "latest.log".to_string(),
        }
    }
}
