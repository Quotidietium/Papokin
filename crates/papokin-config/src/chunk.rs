use std::str;

use serde::{Deserialize, Serialize};

/// 区块存储格式的配置。
///
/// 支持多种区块格式，目前为 `Anvil` 和 `Linear`。
#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum ChunkConfig {
    /// 标准 Anvil 区块存储。
    #[serde(rename = "anvil")]
    Anvil(AnvilChunkConfig),
    /// Linear 区块存储格式。
    #[serde(rename = "linear")]
    Linear,
    /// Pumpkin 自有的优化世界格式。
    #[serde(rename = "pump")]
    Pump,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self::Anvil(AnvilChunkConfig::default())
    }
}

/// Anvil 区块存储的配置。
#[derive(Deserialize, Serialize, Default, Clone)]
#[serde(default)]
pub struct AnvilChunkConfig {
    /// 区块数据的压缩设置。
    pub compression: ChunkCompression,
    /// 区块是否应就地写入。
    pub write_in_place: bool,
}

/// 区块数据的压缩设置。
#[derive(Deserialize, Serialize, Clone)]
pub struct ChunkCompression {
    /// 要使用的压缩算法。
    pub algorithm: Compression,
    /// 压缩级别（因算法而异）。
    pub level: u32,
}

impl Default for ChunkCompression {
    fn default() -> Self {
        // ZLib 与原版/Papo 的默认值相同，使新写入的区域
        // 与原版服务器字节兼容的文件。
        Self {
            algorithm: Compression::ZLib,
            level: 6,
        }
    }
}

/// 用于区块数据存储的压缩算法。
#[derive(Deserialize, Serialize, Clone, Copy)]
pub enum Compression {
    /// `GZip` 压缩。
    GZip,
    /// `ZLib` 压缩。
    ZLib,
    /// LZ4 压缩（自 24w04a 起）。
    LZ4,
    /// 自定义压缩算法（自 24w05a 起）。
    Custom,
}
