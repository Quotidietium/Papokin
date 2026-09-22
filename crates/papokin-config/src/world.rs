use serde::{Deserialize, Serialize};

use crate::{chunk::ChunkConfig, lighting::LightingEngineConfig};

/// 世界与关卡特定设置的配置。
///
/// 目前包含与区块相关的选项；之后可能会添加更多设置。
#[derive(Deserialize, Serialize, Clone)]
pub struct LevelConfig {
    /// 区块行为与管理的配置。
    pub chunk: ChunkConfig,
    /// 光照引擎传播模式的配置。
    #[serde(default)]
    pub lighting: LightingEngineConfig,
    /// 自动保存检查之间的刻数。若为 0，则禁用自动保存。
    #[serde(default = "default_autosave_ticks")]
    pub autosave_ticks: u64,
    // TODO: 更多选项
}

const fn default_autosave_ticks() -> u64 {
    6000 // 按 20 TPS 默认为 5 分钟
}

impl Default for LevelConfig {
    fn default() -> Self {
        Self {
            chunk: ChunkConfig::default(),
            lighting: LightingEngineConfig::default(),
            autosave_ticks: default_autosave_ticks(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_level_config_enables_autosave() {
        assert_eq!(LevelConfig::default().autosave_ticks, 6000);
    }

    #[test]
    fn generated_config_round_trip_keeps_autosave_enabled() {
        let generated = toml::to_string(&LevelConfig::default()).unwrap();
        let reloaded: LevelConfig = toml::from_str(&generated).unwrap();
        assert_eq!(reloaded.autosave_ticks, 6000);
    }

    #[test]
    fn an_explicit_zero_still_disables_autosave() {
        let disabled = LevelConfig {
            autosave_ticks: 0,
            ..LevelConfig::default()
        };
        let generated = toml::to_string(&disabled).unwrap();
        let reloaded: LevelConfig = toml::from_str(&generated).unwrap();
        assert_eq!(reloaded.autosave_ticks, 0);
    }
}
