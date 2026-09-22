use serde::{Deserialize, Serialize};

/// 客户端数据包速率限制的配置。
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(default)]
pub struct PacketLimiterConfig {
    /// 是否启用数据包速率限制器。
    pub enabled: bool,
    /// 每个客户端每秒允许传入的数据包（网络）最大数量。
    /// 值 <= 0.0 时禁用速率限制。
    #[serde(alias = "max-packet-rate")]
    pub max_packet_rate: f64,
    /// 数据包限速的突发允许容量。
    #[serde(alias = "burst-capacity")]
    pub burst_capacity: f64,
    /// 客户端超过数据包速率限制时的踢出消息。
    #[serde(alias = "kick-message")]
    pub kick_message: String,
}

impl Default for PacketLimiterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_packet_rate: 500.0,
            burst_capacity: 500.0,
            kick_message: "因发送数据包过于频繁被踢出".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_parsing_with_kebab_case() {
        let toml_str = r#"
            enabled = true
            max-packet-rate = 300.0
            burst-capacity = 100.0
            kick-message = "Stop spamming"
        "#;
        let config: PacketLimiterConfig = toml::from_str(toml_str).unwrap();
        assert!(config.enabled);
        assert!((config.max_packet_rate - 300.0).abs() < f64::EPSILON);
        assert!((config.burst_capacity - 100.0).abs() < f64::EPSILON);
        assert_eq!(config.kick_message, "Stop spamming");
    }

    #[test]
    fn toml_parsing_with_snake_case() {
        let toml_str = r#"
            enabled = true
            max_packet_rate = 500.0
            burst_capacity = 500.0
            kick_message = "Kicked for spamming packets"
        "#;
        let config: PacketLimiterConfig = toml::from_str(toml_str).unwrap();
        assert!(config.enabled);
        assert!((config.max_packet_rate - 500.0).abs() < f64::EPSILON);
        assert!((config.burst_capacity - 500.0).abs() < f64::EPSILON);
        assert_eq!(config.kick_message, "Kicked for spamming packets");
    }
}
