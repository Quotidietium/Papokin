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
#[derive(Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct AnvilChunkConfig {
    /// 区块数据的压缩设置。
    pub compression: ChunkCompression,
    /// 区块是否应就地写入。
    ///
    /// 默认开启：就地路径只重写脏区块所在的扇区 + 头部时间戳
    /// （容量变化或新区块仍走原子化整体重写兜底），单区块自动
    /// 保存从整 region 数 MiB 重写降到数 KiB——高负载下磁盘写
    /// 放大与占用时长相差三个数量级。就地写崩溃窗口的兜底与原版
    /// 相同：读侧对越界/解析失败条目自愈丢弃（见 anvil.rs
    /// `read`），最坏损失正在写入的单个区块。
    pub write_in_place: bool,
}

impl Default for AnvilChunkConfig {
    fn default() -> Self {
        Self {
            compression: ChunkCompression::default(),
            write_in_place: true,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anvil_default_uses_in_place_writes() {
        // 默认必须启用就地写入：否则每次保存任一区块都会升格为
        // 整 region 原子重写（写放大多个数量级）
        assert!(AnvilChunkConfig::default().write_in_place);
    }
}
