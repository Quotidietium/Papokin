use serde::{Deserialize, Serialize};

/// 游戏内聊天行为的配置。
///
/// 控制聊天格式、显示和防刷屏保护。
#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct ChatConfig {
    /// 自定义聊天格式。
    /// `Note`：启用安全聊天时不适用。
    pub format: String,
    /// 玩家聊天与命令的防刷屏保护设置。
    pub anti_spam: AntiSpamConfig,
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            format: "<{DISPLAYNAME}> {MESSAGE}".to_string(),
            anti_spam: AntiSpamConfig::default(),
        }
    }
}

/// 聊天与命令防刷屏保护的配置。
#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct AntiSpamConfig {
    /// 是否启用防刷屏保护。
    pub enabled: bool,
    /// 旧版基于刻的刷屏阈值。为兼容性而保留。
    pub spam_threshold: u32,
    /// 聊天刷屏阈值（秒）。设置后将覆盖旧式的刻阈值。
    #[serde(
        alias = "chat-spam-threshold-seconds",
        alias = "chat_spam_threshold_seconds"
    )]
    pub chat_spam_threshold_seconds: Option<u32>,
    /// 命令刷屏阈值（秒）。设置后将覆盖旧式的刻阈值。
    #[serde(
        alias = "command-spam-threshold-seconds",
        alias = "command_spam_threshold_seconds"
    )]
    pub command_spam_threshold_seconds: Option<u32>,
    /// 每发送一条聊天消息或命令时累加到刷屏计数器上的量。
    /// 原版默认为 20 刻。
    pub message_cost: u32,
    /// 每个服务器刻从刷屏计数器中衰减的量。
    /// 原版默认为 1 刻。
    pub decay_per_tick: u32,
    /// 管理员是否绕过防刷屏检查。
    pub ops_bypass: bool,
}

impl AntiSpamConfig {
    /// 解析生效的聊天阈值（以刻为单位）。
    #[must_use]
    pub fn chat_threshold_ticks(&self) -> u32 {
        self.chat_spam_threshold_seconds
            .map_or(self.spam_threshold, |seconds| seconds.saturating_mul(20))
    }

    /// 解析生效的命令阈值（以刻为单位）。
    #[must_use]
    pub fn command_threshold_ticks(&self) -> u32 {
        self.command_spam_threshold_seconds
            .map_or(self.spam_threshold, |seconds| seconds.saturating_mul(20))
    }
}

impl Default for AntiSpamConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            spam_threshold: 200,
            chat_spam_threshold_seconds: None,
            command_spam_threshold_seconds: None,
            message_cost: 20,
            decay_per_tick: 1,
            ops_bypass: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_chat_config() {
        let config = ChatConfig::default();
        assert_eq!(config.format, "<{DISPLAYNAME}> {MESSAGE}");
        assert!(config.anti_spam.enabled);
        assert_eq!(config.anti_spam.spam_threshold, 200);
        assert_eq!(config.anti_spam.chat_spam_threshold_seconds, None);
        assert_eq!(config.anti_spam.command_spam_threshold_seconds, None);
        assert_eq!(config.anti_spam.message_cost, 20);
        assert_eq!(config.anti_spam.decay_per_tick, 1);
        assert!(config.anti_spam.ops_bypass);
    }

    #[test]
    fn deserialize_custom_anti_spam() {
        let toml_str = r#"
            format = "<{NAME}> {MESSAGE}"
            [anti_spam]
            enabled = false
            spam_threshold = 100
            chat-spam-threshold-seconds = 5
            command-spam-threshold-seconds = 7
            message_cost = 10
            decay_per_tick = 2
            ops_bypass = false
        "#;
        let config: ChatConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.format, "<{NAME}> {MESSAGE}");
        assert!(!config.anti_spam.enabled);
        assert_eq!(config.anti_spam.spam_threshold, 100);
        assert_eq!(config.anti_spam.chat_spam_threshold_seconds, Some(5));
        assert_eq!(config.anti_spam.command_spam_threshold_seconds, Some(7));
        assert_eq!(config.anti_spam.message_cost, 10);
        assert_eq!(config.anti_spam.decay_per_tick, 2);
        assert!(!config.anti_spam.ops_bypass);
        assert_eq!(config.anti_spam.chat_threshold_ticks(), 100);
        assert_eq!(config.anti_spam.command_threshold_ticks(), 140);
    }
}
