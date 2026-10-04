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
    /// 单个区块序列化器缓存（region/entities 各一份）驻留内存的
    /// 上限，单位 MiB。超限后按最近最少使用驱逐*磁盘已是最新*
    /// 的缓存条目（有未落盘更新的条目绝不驱逐，数据安全性不受
    /// 影响）；被驱逐条目下次访问时从磁盘重读。0 表示不设上限
    /// （恢复 0.3.14 及之前的旧行为）。
    #[serde(default = "default_cache_max_mb")]
    pub cache_max_mb: u32,
    // TODO: 更多选项
}

const fn default_autosave_ticks() -> u64 {
    6000 // 按 20 TPS 默认为 5 分钟
}

const fn default_cache_max_mb() -> u32 {
    256
}

impl Default for LevelConfig {
    fn default() -> Self {
        Self {
            chunk: ChunkConfig::default(),
            lighting: LightingEngineConfig::default(),
            autosave_ticks: default_autosave_ticks(),
            cache_max_mb: default_cache_max_mb(),
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

    #[test]
    fn default_level_config_bounds_serializer_cache() {
        // 默认必须设上限：无界缓存会让 watched region 的压缩字节
        // 无限驻留内存（见 note/report/perf 轮次 1）
        assert_eq!(LevelConfig::default().cache_max_mb, 256);
    }

    #[test]
    fn cache_max_mb_round_trip_and_zero_unlimited() {
        let generated = toml::to_string(&LevelConfig::default()).unwrap();
        let reloaded: LevelConfig = toml::from_str(&generated).unwrap();
        assert_eq!(reloaded.cache_max_mb, 256);

        let unlimited = LevelConfig {
            cache_max_mb: 0,
            ..LevelConfig::default()
        };
        let generated = toml::to_string(&unlimited).unwrap();
        let reloaded: LevelConfig = toml::from_str(&generated).unwrap();
        assert_eq!(reloaded.cache_max_mb, 0);
    }
}
