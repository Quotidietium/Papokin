use serde::{Deserialize, Serialize};

/// 遥测配置选项。
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(default)]
pub struct TelemetryConfig {
    /// 是否启用匿名遥测。默认为 true。
    pub enabled: bool,
    /// 自定义遥测后端的接入端点。
    pub endpoint: String,
    /// 心跳间隔（秒）（默认：300 秒 / 5 分钟）。最小 60 秒。
    pub interval_secs: u64,
    /// 是否选择加入公开社区目录以展示此服务器。
    pub public: bool,
    /// 如果 public 为 true，则在分析仪表板上显示的公开服务器名称。
    pub server_name: Option<String>,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: "https://market.pumpkinmc.org/api/v1/rest/telemetry/heartbeat".to_string(),
            interval_secs: 300,
            public: false,
            server_name: None,
        }
    }
}

impl TelemetryConfig {
    /// 验证遥测配置选项。
    pub const fn validate(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_default() {
        let default_config = TelemetryConfig::default();
        assert!(default_config.enabled);
        assert_eq!(
            default_config.endpoint,
            "https://market.pumpkinmc.org/api/v1/rest/telemetry/heartbeat"
        );
        assert_eq!(default_config.interval_secs, 300);
        assert!(!default_config.public);
        assert_eq!(default_config.server_name, None);
    }

    #[test]
    fn telemetry_toml_deserialization() {
        let toml_str = r#"
            enabled = false
            endpoint = "http://localhost:5000/api/v1/rest/telemetry/heartbeat"
            interval_secs = 60
            public = true
            server_name = "Test SMP"
        "#;

        let config: TelemetryConfig = toml::from_str(toml_str).unwrap();
        assert!(!config.enabled);
        assert_eq!(
            config.endpoint,
            "http://localhost:5000/api/v1/rest/telemetry/heartbeat"
        );
        assert_eq!(config.interval_secs, 60);
        assert!(config.public);
        assert_eq!(config.server_name.as_deref(), Some("Test SMP"));
    }
}
