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
            // 默认禁用：Papokin 是独立衍生项目，没有可用的官方遥测
            // 后端，不得默认把服务器心跳发往原版组织的端点。
            enabled: false,
            endpoint: String::new(),
            interval_secs: 300,
            public: false,
            server_name: None,
        }
    }
}

impl TelemetryConfig {
    /// 验证遥测配置选项。
    ///
    /// # Panics
    ///
    /// 启用心跳间隔小于 60 秒，或已启用但接入端点为空时 panic，
    /// 与其余配置校验一致地在启动期暴露错误配置。
    pub fn validate(&self) {
        assert!(
            self.interval_secs >= 60,
            "遥测心跳间隔不得小于 60 秒（当前 {} 秒）",
            self.interval_secs
        );
        assert!(
            !self.enabled || !self.endpoint.trim().is_empty(),
            "遥测已启用但 endpoint 为空：请填写接入端点或在配置中禁用遥测"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_default() {
        let default_config = TelemetryConfig::default();
        // 默认禁用且端点为空：衍生项目不得默认外联原版组织的遥测端点
        assert!(!default_config.enabled);
        assert!(default_config.endpoint.is_empty());
        assert_eq!(default_config.interval_secs, 300);
        assert!(!default_config.public);
        assert_eq!(default_config.server_name, None);
        default_config.validate();
    }

    #[test]
    fn telemetry_rejects_short_interval_and_empty_endpoint() {
        let mut config = TelemetryConfig {
            interval_secs: 30,
            ..TelemetryConfig::default()
        };
        assert!(std::panic::catch_unwind(|| config.validate()).is_err());

        let mut config = TelemetryConfig {
            enabled: true,
            endpoint: String::new(),
            ..TelemetryConfig::default()
        };
        assert!(std::panic::catch_unwind(|| config.validate()).is_err());

        config.endpoint = "https://example.invalid/heartbeat".to_string();
        config.validate();
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
