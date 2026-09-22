use serde::{Deserialize, Serialize};

/// 数据包压缩的配置。
///
/// 控制是否启用网络数据包压缩以及压缩参数。
#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct CompressionConfig {
    /// 是否启用压缩。
    pub enabled: bool,
    /// 详细的压缩设置。
    #[serde(flatten)]
    pub info: CompressionInfo,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            info: CompressionInfo::default(),
        }
    }
}

/// 网络数据包压缩设置的详细信息。
///
/// 也可以独立于配置单独使用。
#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct CompressionInfo {
    /// 压缩阈值（以字节为单位）。
    /// 小于该值的数据包不会被压缩。
    pub threshold: u32,
    /// 压缩级别，范围为 `0..9`。
    /// `1` = 针对速度优化，`9` = 针对大小优化。
    pub level: u32,
}

impl Default for CompressionInfo {
    fn default() -> Self {
        Self {
            threshold: 256,
            level: 4,
        }
    }
}
