use bytes::{Buf, BufMut, Bytes};
use flate2::read::{GzDecoder, GzEncoder, ZlibDecoder, ZlibEncoder};
use lz4_java_wrc::Context;
use papokin_config::chunk::AnvilChunkConfig;
use papokin_util::math::vector2::Vector2;
use std::{
    io::{Read, SeekFrom, Write},
    marker::PhantomData,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncSeekExt, AsyncWrite, AsyncWriteExt, BufWriter},
    sync::Mutex,
};
use tracing::{debug, trace, warn};

use crate::chunk::{
    ChunkParsingError, ChunkReadingError, ChunkSerializingError, ChunkWritingError,
    CompressionError,
    io::{ChunkSerializer, Dirtiable, LoadedData, atomic_write, run_blocking},
};

/// 区域的边长（以区块计，一个区域为 32x32 区块）
pub const REGION_SIZE: usize = 32;

/// 用于区分同一区域内各个区块的位数
pub const SUBREGION_BITS: u8 = papokin_util::math::ceil_log2(REGION_SIZE as u32);

pub const SUBREGION_AND: i32 = i32::pow(2, SUBREGION_BITS as u32) - 1;

/// 一个区域内的区块数量
pub const CHUNK_COUNT: usize = REGION_SIZE * REGION_SIZE;

/// 一个扇区的字节数（4 KiB）
const SECTOR_BYTES: usize = 4096;

/// 位置条目的单字节计数字段所能表示的最大扇区数。需要更多扇区的区块
/// 会像原版一样存为外部区块（`.mcc`），因此该字段永远不会溢出。
const MAX_IN_FILE_SECTORS: u32 = 255;

/// 当区块负载存放在外部 `c.<x>.<z>.mcc` 文件中时，按位或进压缩版本字节的
/// 标志位（原版外部区块机制）。
const EXTERNAL_FLAG: u8 = 0x80;

/// 加载时需要合并回去的超大复合标签的键（兼容 Paper 的
/// `Allow Saving of Oversized Chunks`）。
const OVERSIZED_MERGE_KEYS: [&str; 3] = ["Entities", "block_entities", "TileEntities"];

// 1.21.11
pub const WORLD_DATA_VERSION: i32 = 4671;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Compression {
    /// `GZip` 压缩
    GZip = Self::GZIP_ID,
    /// `ZLib` 压缩
    ZLib = Self::ZLIB_ID,
    /// LZ4 压缩（自 24w04a 起）
    LZ4 = Self::LZ4_ID,
    /// 自定义压缩算法（自 24w05a 起）
    Custom = Self::CUSTOM_ID,
}

pub enum CompressionRead<R: Read> {
    GZip(GzDecoder<R>),
    ZLib(ZlibDecoder<R>),
    LZ4(lz4_java_wrc::Lz4BlockInput<R>),
}

impl<R: Read> Read for CompressionRead<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::GZip(gzip) => gzip.read(buf),
            Self::ZLib(zlib) => zlib.read(buf),
            Self::LZ4(lz4) => lz4.read(buf),
        }
    }
}

#[derive(Clone)]
pub struct AnvilChunkData {
    compression: Option<Compression>,
    // 长度始终是该数据长度 + 压缩字节（1），因此无需单独保存长度
    compressed_data: Bytes,
    /// 负载存放在外部 `c.<x>.<z>.mcc` 文件中；区域文件只保存五字节头部。
    /// 从磁盘读取此类区块时，负载由 [`AnvilChunkFile::read_at`] 填充。
    external: bool,
}

enum WriteAction {
    // 不写入任何内容
    Pass,
    // 写入整个文件
    All,
    // 只写入特定索引
    Parts(Vec<usize>),
}

impl WriteAction {
    /// 如果当前没有写入，设置为新的 Parts 变体；
    /// 如果已是 Parts 变体，则向其追加；
    /// 如果已是 All 变体，则什么也不做
    fn maybe_update_chunk_index(&mut self, index: usize) {
        match self {
            Self::Pass => *self = Self::Parts(vec![index]),
            Self::Parts(parts) => {
                if !parts.contains(&index) {
                    parts.push(index);
                }
            }
            Self::All => {}
        }
    }
}

struct AnvilChunkMetadata {
    serialized_data: AnvilChunkData,
    timestamp: u32,

    // NOTE: 仅当 WriteAction 为 `Parts` 时该字段才有效
    file_sector_offset: u32,
}

pub struct AnvilChunkFile<S: SingleChunkDataSerializer> {
    chunks_data: [Option<AnvilChunkMetadata>; CHUNK_COUNT],
    end_sector: u32,
    write_action: Mutex<WriteAction>,

    _dummy: PhantomData<S>,
}

impl Compression {
    const GZIP_ID: u8 = 1;
    const ZLIB_ID: u8 = 2;
    const NO_COMPRESSION_ID: u8 = 3;
    const LZ4_ID: u8 = 4;
    const CUSTOM_ID: u8 = 127;

    /// 单区块解压输出上限。正常区块解压后远小于此值；不设上限时，
    /// 恶意压缩数据（解压炸弹）可借 zlib ~1032:1 的膨胀比把 1 MB
    /// 输入放大到 GB 级直至内存耗尽。与原版解压后大小限制同量级。
    const MAX_DECOMPRESSED_LEN: usize = 8 * 1024 * 1024;

    fn decompress_data(self, compressed_data: &[u8]) -> Result<Box<[u8]>, CompressionError> {
        fn decode<R: std::io::Read>(reader: R, capacity: usize) -> std::io::Result<Box<[u8]>> {
            let mut buf = Vec::with_capacity(capacity.min(Compression::MAX_DECOMPRESSED_LEN));
            let mut bounded = reader.take((Compression::MAX_DECOMPRESSED_LEN + 1) as u64);
            bounded.read_to_end(&mut buf)?;
            if buf.len() > Compression::MAX_DECOMPRESSED_LEN {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "解压输出超过单区块上限（疑似解压炸弹）",
                ));
            }
            Ok(buf.into_boxed_slice())
        }

        let initial_capacity = compressed_data.len();

        match self {
            Self::GZip => decode(GzDecoder::new(compressed_data), initial_capacity)
                .map_err(CompressionError::GZipError),
            Self::ZLib => decode(ZlibDecoder::new(compressed_data), initial_capacity)
                .map_err(CompressionError::ZlibError),
            Self::LZ4 => decode(
                lz4_java_wrc::Lz4BlockInput::new(compressed_data),
                initial_capacity,
            )
            .map_err(CompressionError::LZ4Error),
            Self::Custom => Err(CompressionError::UnknownCompression),
        }
    }

    const LZ4_COMPRESSION_LEVEL_BASE: u32 = 10;
    fn compress_data(
        self,
        uncompressed_data: &[u8],
        compression_level: u32,
    ) -> Result<Vec<u8>, CompressionError> {
        match self {
            Self::GZip => {
                let mut encoder = GzEncoder::new(
                    uncompressed_data,
                    flate2::Compression::new(compression_level),
                );
                let mut chunk_data = Vec::new();
                encoder
                    .read_to_end(&mut chunk_data)
                    .map_err(CompressionError::GZipError)?;
                Ok(chunk_data)
            }
            Self::ZLib => {
                let mut encoder = ZlibEncoder::new(
                    uncompressed_data,
                    flate2::Compression::new(compression_level),
                );
                let mut chunk_data = Vec::new();
                encoder
                    .read_to_end(&mut chunk_data)
                    .map_err(CompressionError::ZlibError)?;
                Ok(chunk_data)
            }
            Self::LZ4 => {
                let mut compressed_data = Vec::new();
                let block_size = 1 << (Self::LZ4_COMPRESSION_LEVEL_BASE + compression_level);
                let mut encoder = lz4_java_wrc::Lz4BlockOutput::with_context(
                    &mut compressed_data,
                    Context::default(),
                    block_size,
                )
                .map_err(CompressionError::LZ4Error)?;
                encoder
                    .write_all(uncompressed_data)
                    .map_err(CompressionError::LZ4Error)?;
                drop(encoder);
                Ok(compressed_data)
            }
            Self::Custom => Err(CompressionError::UnknownCompression),
        }
    }

    /// 找到对应压缩方式时返回 Ok，否则返回 Err
    #[expect(clippy::result_unit_err)]
    pub const fn from_byte(byte: u8) -> Result<Option<Self>, ()> {
        match byte {
            Self::GZIP_ID => Ok(Some(Self::GZip)),
            Self::ZLIB_ID => Ok(Some(Self::ZLib)),
            // 未压缩（自 1.15.1 之前的某个版本起）
            Self::NO_COMPRESSION_ID => Ok(None),
            Self::LZ4_ID => Ok(Some(Self::LZ4)),
            Self::CUSTOM_ID => Ok(Some(Self::Custom)),
            // 未知格式
            _ => Err(()),
        }
    }
}

impl From<papokin_config::chunk::Compression> for Compression {
    fn from(value: papokin_config::chunk::Compression) -> Self {
        // :c
        match value {
            papokin_config::chunk::Compression::GZip => Self::GZip,
            papokin_config::chunk::Compression::ZLib => Self::ZLib,
            papokin_config::chunk::Compression::LZ4 => Self::LZ4,
            papokin_config::chunk::Compression::Custom => Self::Custom,
        }
    }
}

impl AnvilChunkData {
    /// 序列化区块的原始大小
    #[inline]
    const fn raw_write_size(&self) -> usize {
        if self.external {
            // 只有五字节头部会写入区域文件
            return 4 + 1;
        }
        // 4 字节表示*长度*，1 字节表示*压缩*方式
        self.compressed_data.len() + 4 + 1
    }

    /// 含填充的序列化区块大小
    #[inline]
    const fn padded_size(&self) -> usize {
        let sector_count = self.sector_count() as usize;
        sector_count * SECTOR_BYTES
    }

    /// 该区块在区域文件内占用的扇区数。外部区块只占用一个扇区，
    /// 用于存放五字节头部。
    #[inline]
    const fn sector_count(&self) -> u32 {
        if self.external {
            return 1;
        }
        let total_size = self.raw_write_size();
        total_size.div_ceil(SECTOR_BYTES) as u32
    }

    /// 负载是否过大而无法存储在区域文件内。
    #[inline]
    const fn is_oversized(&self) -> bool {
        !self.external && self.sector_count() > MAX_IN_FILE_SECTORS
    }

    fn from_bytes(bytes: Bytes) -> Result<Self, ChunkReadingError> {
        let mut bytes = bytes;
        // 减一去掉长度所包含的压缩字节，因此小于一的值根本无法描述一个区块。
        let declared_length = bytes.get_u32() as usize;
        let Some(length) = declared_length.checked_sub(1) else {
            return Err(ChunkReadingError::ParsingError(
                ChunkParsingError::ErrorDeserializingChunk(
                    "Chunk length does not cover its compression byte".to_string(),
                ),
            ));
        };

        let compression_method = bytes.get_u8();
        let external = compression_method & EXTERNAL_FLAG != 0;
        let compression = Compression::from_byte(compression_method & !EXTERNAL_FLAG)
            .map_err(|()| ChunkReadingError::Compression(CompressionError::UnknownCompression))?;

        if external {
            // 负载存放在外部文件中；区域文件只保存五字节头部。
            // 这里无需截取任何数据。
            return Ok(Self {
                compression,
                compressed_data: Bytes::new(),
                external: true,
            });
        }

        if length > bytes.len() {
            return Err(ChunkReadingError::ParsingError(
                ChunkParsingError::ErrorDeserializingChunk(format!(
                    "Chunk length is greater than available bytes ({} vs {})",
                    length,
                    bytes.len()
                )),
            ));
        }

        Ok(Self {
            compression,
            // 如果有填充，需要将其裁剪掉
            compressed_data: bytes.slice(..length),
            external: false,
        })
    }

    async fn write(&self, w: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), std::io::Error> {
        let padded_size = self.padded_size();

        // 声明的长度总是涵盖负载加上压缩
        // 字节，即使负载本身存储在外部文件中。
        w.write_u32((self.compressed_data.remaining() + 1) as u32)
            .await?;
        let version_byte = self
            .compression
            .map_or(Compression::NO_COMPRESSION_ID, |c| c as u8);
        let version_byte = if self.external {
            version_byte | EXTERNAL_FLAG
        } else {
            version_byte
        };
        w.write_u8(version_byte).await?;

        if !self.external {
            w.write_all(&self.compressed_data).await?;
        }

        let padding_len = padded_size - self.raw_write_size();
        if padding_len > 0 {
            static PADDING: [u8; SECTOR_BYTES] = [0; SECTOR_BYTES];
            w.write_all(&PADDING[..padding_len]).await?;
        }

        Ok(())
    }

    fn to_chunk<S>(&self, pos: Vector2<i32>) -> Result<S, ChunkReadingError>
    where
        S: SingleChunkDataSerializer,
    {
        if let Some(compression) = self.compression {
            let decompress_bytes = compression
                .decompress_data(&self.compressed_data)
                .map_err(ChunkReadingError::Compression)?;

            S::from_bytes(&decompress_bytes.into(), pos)
        } else {
            S::from_bytes(&self.compressed_data, pos)
        }
    }

    fn from_chunk<S>(
        chunk: &S,
        compression: Option<Compression>,
        chunk_config: &AnvilChunkConfig,
    ) -> Result<Self, ChunkWritingError>
    where
        S: SingleChunkDataSerializer,
    {
        let raw_bytes = chunk
            .to_bytes()
            .map_err(|err| ChunkWritingError::ChunkSerializingError(err.to_string()))?;

        let compression = compression.unwrap_or_else(|| chunk_config.compression.algorithm.into());
        let level = chunk_config.compression.level;
        let compressed_data = compression
            .compress_data(&raw_bytes, level)
            .map_err(ChunkWritingError::Compression)?;

        Ok(Self {
            compression: Some(compression),
            compressed_data: compressed_data.into(),
            external: false,
        })
    }
}

/// 从 `r.<x>.<z>.mca` 文件名中解析出区域坐标。
fn region_coords_from_path(path: &Path) -> Option<(i32, i32)> {
    let name = path.file_name()?.to_str()?;
    let stem = name.strip_suffix(".mca")?;
    let mut parts = stem.split('.');
    if parts.next()? != "r" {
        return None;
    }
    let x: i32 = parts.next()?.parse().ok()?;
    let z: i32 = parts.next()?.parse().ok()?;
    Some((x, z))
}

/// 绝对区块位置对应的原版外部区块文件的路径。
fn external_chunk_path(region_path: &Path, abs_x: i32, abs_z: i32) -> PathBuf {
    region_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("c.{abs_x}.{abs_z}.mcc"))
}

/// 绝对区块对应的 Paper 超大区块伴随文件的路径
/// 位置（`Allow Saving of Oversized Chunks`）。
fn paper_oversized_chunk_path(region_path: &Path, abs_x: i32, abs_z: i32) -> Option<PathBuf> {
    let name = region_path.file_name()?.to_str()?.strip_suffix(".mca")?;
    Some(
        region_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("{name}_oversized_{abs_x}_{abs_z}.nbt")),
    )
}

/// 整个区域的 Paper 超大区块位图伴随文件的路径。
fn paper_oversized_meta_path(region_path: &Path) -> Option<PathBuf> {
    let name = region_path.file_name()?.to_str()?.strip_suffix(".mca")?;
    Some(
        region_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("{name}.oversized.nbt")),
    )
}

/// 读取 Paper 的超尺寸位图，返回 `[bool; CHUNK_COUNT]`。
fn read_paper_oversized_meta(region_path: &Path) -> [bool; CHUNK_COUNT] {
    let mut flags = [false; CHUNK_COUNT];
    let Some(meta_path) = paper_oversized_meta_path(region_path) else {
        return flags;
    };
    match std::fs::read(&meta_path) {
        Ok(bytes) => {
            for (index, flag) in flags.iter_mut().enumerate() {
                *flag = bytes.get(index).is_some_and(|byte| *byte == 1);
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            warn!("读取超大区块元数据文件 {} 失败：{err}", meta_path.display());
        }
    }
    flags
}

/// 将超限的旁挂（sidecar）复合标签合并进基础区块 NBT。
fn merge_paper_oversized(
    base_nbt: &mut papokin_nbt::NbtCompound,
    oversized: &papokin_nbt::NbtCompound,
) {
    for key in OVERSIZED_MERGE_KEYS {
        let Some(extra) = oversized.get_list(key) else {
            continue;
        };
        if extra.is_empty() {
            continue;
        }
        let extra = extra.to_vec();
        match base_nbt.get_list(key) {
            Some(existing)
                if matches!(extra.first(), Some(papokin_nbt::tag::NbtTag::Compound(_))) =>
            {
                let mut merged = existing.to_vec();
                merged.extend(extra);
                base_nbt.put_list(key, merged);
            }
            _ => {
                base_nbt.put_list(key, extra);
            }
        }
    }
}

impl<S: SingleChunkDataSerializer> AnvilChunkFile<S> {
    #[must_use]
    pub const fn get_region_coords(at: &Vector2<i32>) -> (i32, i32) {
        // 除以 32 得到 region 坐标
        (at.x >> SUBREGION_BITS, at.y >> SUBREGION_BITS)
    }

    #[must_use]
    pub const fn get_chunk_index(x: i32, z: i32) -> usize {
        let local_x = x & SUBREGION_AND;
        let local_z = z & SUBREGION_AND;
        let index = (local_z << SUBREGION_BITS) + local_x;
        index as usize
    }

    /// 给定区域内某个区块索引的绝对区块坐标
    /// 路径所指向的内容。
    fn absolute_chunk_pos(region_path: &Path, index: usize) -> Option<(i32, i32)> {
        let (region_x, region_z) = region_coords_from_path(region_path)?;
        let local_x = (index % REGION_SIZE) as i32;
        let local_z = (index / REGION_SIZE) as i32;
        Some((
            region_x * REGION_SIZE as i32 + local_x,
            region_z * REGION_SIZE as i32 + local_z,
        ))
    }

    /// 若存在，则读取区块的外部 `.mcc` 负载数据。
    fn read_external_payload(region_path: &Path, index: usize) -> Option<Bytes> {
        let (abs_x, abs_z) = Self::absolute_chunk_pos(region_path, index)?;
        match std::fs::read(external_chunk_path(region_path, abs_x, abs_z)) {
            Ok(data) => Some(data.into()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => {
                warn!("读取区块 {abs_x}/{abs_z} 的外部区块文件失败：{err}");
                None
            }
        }
    }

    /// 为按外部方式读取的区块，从其 `.mcc` 文件填充负载。
    /// 缺失文件会使负载为空，从而表现为每区块的，
    /// 错误，而不是让整个区域失败。
    fn fill_external_payloads(&mut self, region_path: &Path) {
        for (index, metadata) in self.chunks_data.iter_mut().enumerate() {
            let Some(metadata) = metadata else {
                continue;
            };
            if metadata.serialized_data.external
                && metadata.serialized_data.compressed_data.is_empty()
            {
                if let Some(payload) = Self::read_external_payload(region_path, index) {
                    metadata.serialized_data.compressed_data = payload;
                } else {
                    warn!(
                        "{} 中索引 {index} 的外部区块文件缺失；该区块\
                         无法加载",
                        region_path.display()
                    );
                }
            }
        }
    }

    /// 如果存在，则将 Paper 超大附属数据应用到刚解析的区域上，
    /// 任何情况。被标记为超大的区块会将其实体数据合并回
    /// 基础负载，让流水线的其余部分看到一个正常区块。
    fn apply_paper_oversized(&mut self, region_path: &Path) {
        let flags = read_paper_oversized_meta(region_path);
        if !flags.iter().any(|flag| *flag) {
            return;
        }

        for (index, flagged) in flags.into_iter().enumerate() {
            if !flagged {
                continue;
            }
            let Some(metadata) = self.chunks_data[index].as_mut() else {
                continue;
            };
            let Some((abs_x, abs_z)) = Self::absolute_chunk_pos(region_path, index) else {
                continue;
            };
            let Some(oversized_path) = paper_oversized_chunk_path(region_path, abs_x, abs_z) else {
                continue;
            };

            let oversized_nbt = std::fs::read(&oversized_path).and_then(|compressed| {
                // 附属文件是一个经 zlib 压缩、以命名根起始的 NBT 文档；
                // 流式解码器无法 seek，因此先在内存中解码。
                let mut decoded = Vec::new();
                let mut bounded = ZlibDecoder::new(&compressed[..])
                    .take((Compression::MAX_DECOMPRESSED_LEN + 1) as u64);
                bounded
                    .read_to_end(&mut decoded)
                    .map_err(std::io::Error::other)?;
                if decoded.len() > Compression::MAX_DECOMPRESSED_LEN {
                    return Err(std::io::Error::other("解压输出超过单区块上限"));
                }
                let mut cursor = std::io::Cursor::new(decoded);
                let mut reader = papokin_nbt::deserializer::NbtReadHelperJava::new(
                    papokin_nbt::deserializer::NbtStreamReader(&mut cursor),
                );
                papokin_nbt::Nbt::read(&mut reader)
                    .map_err(|err| std::io::Error::other(format!("超大区块 NBT 解析失败：{err}")))
                    .map(|nbt| nbt.root_tag)
            });
            let oversized_nbt = match oversized_nbt {
                Ok(nbt) => nbt,
                Err(err) => {
                    warn!(
                        "读取区块 {abs_x}/{abs_z} 的超大区块数据 {} 失败：{err}",
                        oversized_path.display()
                    );
                    continue;
                }
            };

            // 解压基础负载，合并超大 compound 并
            // 把合并后的原始载荷存回去。
            let merged = metadata
                .serialized_data
                .compression
                .and_then(|compression| {
                    compression
                        .decompress_data(&metadata.serialized_data.compressed_data)
                        .ok()
                        .map(|bytes| Bytes::from(bytes.into_vec()))
                });
            let Some(base_bytes) = merged else {
                continue;
            };

            let mut cursor = std::io::Cursor::new(base_bytes.as_ref());
            let mut reader = papokin_nbt::deserializer::NbtReadHelperJava::new(
                papokin_nbt::deserializer::NbtStreamReader(&mut cursor),
            );
            let Ok(papokin_nbt::Nbt {
                root_tag: mut base_nbt,
                ..
            }) = papokin_nbt::Nbt::read_unnamed(&mut reader)
            else {
                continue;
            };

            merge_paper_oversized(&mut base_nbt, &oversized_nbt);

            metadata.serialized_data = AnvilChunkData {
                compression: None,
                compressed_data: papokin_nbt::Nbt::from(base_nbt).write_unnamed(),
                external: false,
            };
        }
    }

    /// 写入所有外部区块的外部载荷，并清理过期的
    /// 重新内联存储的区块的外部文件。
    async fn write_external_payloads(&self, region_path: &Path) -> Result<(), std::io::Error> {
        for (index, metadata) in self.chunks_data.iter().enumerate() {
            let Some(metadata) = metadata else {
                continue;
            };
            let Some((abs_x, abs_z)) = Self::absolute_chunk_pos(region_path, index) else {
                continue;
            };
            let external_path = external_chunk_path(region_path, abs_x, abs_z);

            if metadata.serialized_data.external
                && !metadata.serialized_data.compressed_data.is_empty()
            {
                atomic_write(&external_path, &metadata.serialized_data.compressed_data).await?;
            } else if !metadata.serialized_data.external {
                // 区块是内联存储的；这个残留的外部文件来自某次
                // 较早的保存会被读取方忽略，但仍将其移除以
                // 避免混淆和浪费磁盘空间。
                match tokio::fs::remove_file(&external_path).await {
                    Ok(()) => {}
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                    Err(err) => {
                        warn!(
                            "删除过期的外部区块文件 {} 失败：{err}",
                            external_path.display()
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// 移除 Paper 的超大区块附属文件（sidecar），这些区块现已改为内联存储。
    /// 在成功写入后调用，以确保超大数据永远不会遮蔽
    /// 新保存的区块被 Paper/Papo 读回时的表现。
    async fn cleanup_paper_oversized(&self, region_path: &Path) {
        let Some(meta_path) = paper_oversized_meta_path(region_path) else {
            return;
        };
        if !meta_path.exists() {
            return;
        }

        for index in 0..CHUNK_COUNT {
            if let Some((abs_x, abs_z)) = Self::absolute_chunk_pos(region_path, index)
                && let Some(path) = paper_oversized_chunk_path(region_path, abs_x, abs_z)
                && let Err(err) = tokio::fs::remove_file(&path).await
                && err.kind() != std::io::ErrorKind::NotFound
            {
                warn!("删除超大区块文件 {} 失败：{err}", path.display());
            }
        }

        if let Err(err) = tokio::fs::remove_file(&meta_path).await
            && err.kind() != std::io::ErrorKind::NotFound
        {
            warn!("删除超大区块元数据文件 {} 失败：{err}", meta_path.display());
        }
    }

    /// 为每个已存在的区块分配全新的连续扇区偏移，以匹配（原版布局），
    /// 与 [`Self::write_all`] 产生的布局完全一致。每当
    /// 写操作会升级为 `All`，从而确保内存中的偏移量永远不会
    /// 描述已不存在的文件布局。
    ///
    /// 直接接收两个字段（而非 `&mut self`），以便调用方可以
    /// 重新编号期间持有 `write_action` 锁。
    fn renumber_sectors(
        chunks_data: &mut [Option<AnvilChunkMetadata>; CHUNK_COUNT],
        end_sector: &mut u32,
    ) {
        let mut next_sector = 2;
        for metadata in chunks_data.iter_mut().flatten() {
            metadata.file_sector_offset = next_sector;
            next_sector += metadata.serialized_data.sector_count();
        }
        *end_sector = next_sector;
    }

    async fn write_indices<I>(&self, path: &Path, indices: I) -> Result<(), std::io::Error>
    where
        I: IntoIterator<Item = usize>,
    {
        trace!("原地写入: {}", path.display());

        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .append(false)
            .open(path)
            .await?;

        let mut write = BufWriter::new(file);
        // 前两个扇区保留给位置表
        let mut header = Vec::with_capacity(SECTOR_BYTES * 2);

        // 位置表
        for metadata in &self.chunks_data {
            if let Some(chunk) = metadata {
                let sector_count = chunk.serialized_data.sector_count();
                debug_assert!(
                    sector_count <= MAX_IN_FILE_SECTORS,
                    "external chunks occupy exactly one sector"
                );
                header.put_u32((chunk.file_sector_offset << 8) | sector_count);
            } else {
                header.put_u32(0);
            }
        }

        // 时间戳表
        for metadata in &self.chunks_data {
            if let Some(chunk) = metadata {
                header.put_u32(chunk.timestamp);
            } else {
                header.put_u32(0);
            }
        }

        // 在单次异步调用中写入全部 8 KiB
        write.write_all(&header).await?;

        let mut chunks = indices
            .into_iter()
            .filter_map(|index| self.chunks_data[index].as_ref().map(|c| (index, c)))
            .collect::<Vec<_>>();

        // 排序使写入有序
        chunks.sort_by_key(|chunk| chunk.1.file_sector_offset);

        #[cfg(debug_assertions)]
        {
            // 验证我们确实位于文件中两个扇区之后
            let current_pos = write.stream_position().await?;
            assert_eq!(current_pos as usize, 2 * SECTOR_BYTES);
        };

        let mut current_sector = 2;
        for (index, chunk) in chunks {
            debug_assert!(
                current_sector <= chunk.file_sector_offset,
                "Current sector is {} but we want to write to {}!",
                current_sector,
                chunk.file_sector_offset
            );

            // 仅在需要时才跳转
            if chunk.file_sector_offset != current_sector {
                trace!("寻址到扇区 {}", chunk.file_sector_offset);
                let _ = write
                    .seek(SeekFrom::Start(
                        chunk.file_sector_offset as u64 * SECTOR_BYTES as u64,
                    ))
                    .await?;
                current_sector = chunk.file_sector_offset;
            }
            trace!(
                "正在写入区块 {} - {}:{}",
                index,
                current_sector,
                chunk.serialized_data.sector_count()
            );

            current_sector += chunk.serialized_data.sector_count();

            chunk.serialized_data.write(&mut write).await?;
        }

        write.flush().await?;
        write.get_ref().sync_all().await?;

        // 外部负载也需要持久化；变为
        // 内联注册的会移除其过期的外部文件。
        self.write_external_payloads(path).await?;

        Ok(())
    }

    /// 写入整个文件，忽略已保存的偏移量。这是安全的默认做法：
    /// 文件会在原文件旁从头构建，并原子地
    /// 原子换入，因此写入中途崩溃也绝不会产生损坏的区域文件。
    async fn write_all(&self, path: &Path) -> Result<(), std::io::Error> {
        let temp_path = path.with_extension("tmp");
        trace!("正在将临时文件写入磁盘: {temp_path:?}");

        let file = tokio::fs::File::create(&temp_path).await?;
        let mut write = BufWriter::new(file);

        // 在内存中构建 8 KiB 头部
        let mut header = Vec::with_capacity(SECTOR_BYTES * 2);
        let mut current_sector: u32 = 2;

        // 位置表
        for metadata in &self.chunks_data {
            if let Some(chunk) = metadata {
                let sector_count = chunk.serialized_data.sector_count();
                debug_assert!(
                    sector_count <= MAX_IN_FILE_SECTORS,
                    "chunks larger than {MAX_IN_FILE_SECTORS} sectors must be external"
                );
                header.put_u32((current_sector << 8) | sector_count);
                current_sector += sector_count;
            } else {
                header.put_u32(0);
            }
        }

        // 时间戳表
        for metadata in &self.chunks_data {
            if let Some(chunk) = metadata {
                header.put_u32(chunk.timestamp);
            } else {
                header.put_u32(0);
            }
        }

        // 在单次异步调用中写入全部 8 KiB
        write.write_all(&header).await?;

        // 写入区块数据
        for chunk in self.chunks_data.iter().flatten() {
            chunk.serialized_data.write(&mut write).await?;
        }

        write.flush().await?;
        write.get_ref().sync_all().await?;

        // 分配的布局必须与 `renumber_sectors` 记录的内容一致；若
        // 若不满足这一点，后续的原地写入会损坏文件。
        // 整条语句都由 cfg 门控：`debug_assert_eq!` 仍会
        // 在调试断言关闭时按名称解析其参数。
        #[cfg(debug_assertions)]
        debug_assert_eq!(
            current_sector,
            2 + self
                .chunks_data
                .iter()
                .flatten()
                .map(|metadata| metadata.serialized_data.sector_count())
                .sum::<u32>()
        );

        // 外部负载会在区域文件之前（原子地）写入
        // 被换入，因此重命名成功即意味着一次完整保存。
        self.write_external_payloads(path).await?;

        tokio::fs::rename(temp_path, path).await?;

        // 由于每个区块再次存入区域文件内，Paper
        // 超长的附属文件必须删除，否则会遮蔽新数据。
        self.cleanup_paper_oversized(path).await;
        Ok(())
    }
}

#[expect(clippy::large_stack_arrays)]
impl<S: SingleChunkDataSerializer> Default for AnvilChunkFile<S> {
    fn default() -> Self {
        Self {
            chunks_data: [const { None }; CHUNK_COUNT],
            write_action: Mutex::new(WriteAction::Pass),
            // 用于偏移量 + 时间戳的两个扇区
            end_sector: 2,
            _dummy: PhantomData,
        }
    }
}

pub trait SingleChunkDataSerializer: Send + Sync + Sized + Dirtiable + 'static {
    fn to_bytes(&self) -> Result<Bytes, ChunkSerializingError>;
    fn from_bytes(bytes: &Bytes, pos: Vector2<i32>) -> Result<Self, ChunkReadingError>;
    fn position(&self) -> (i32, i32);
}

impl<S: SingleChunkDataSerializer> ChunkSerializer for AnvilChunkFile<S> {
    type Data = S;
    type WriteBackend = PathBuf;

    type ChunkConfig = AnvilChunkConfig;

    fn should_write(&self, is_watched: bool) -> bool {
        !is_watched
    }

    fn has_pending_writes(&self) -> bool {
        // Locked 表示当前有写入正在进行；需保守处理。
        self.write_action
            .try_lock()
            .map_or(true, |action| !matches!(&*action, WriteAction::Pass))
    }

    fn get_chunk_key(chunk: &Vector2<i32>) -> String {
        let (region_x, region_z) = Self::get_region_coords(chunk);
        format!("./r.{region_x}.{region_z}.mca")
    }

    async fn write(&self, path: &PathBuf) -> Result<(), std::io::Error> {
        let mut write_action = self.write_action.lock().await;
        match &*write_action {
            WriteAction::Pass => {
                debug!("跳过 {} 的写入，因为没有脏区块", path.display());
                Ok(())
            }
            WriteAction::All => self.write_all(path).await,
            WriteAction::Parts(parts) => self.write_indices(path, parts.iter().copied()).await,
        }?;

        // 如果经过此操作后仍在内存中，就无需再次写入！
        // 失败时上方的 `?` 会提前返回且动作得以保留，因此
        // 下次写入会重试，而不是静默丢弃数据。
        *write_action = WriteAction::Pass;
        Ok(())
    }

    /// 解析区域文件。外部区块负载数据从其
    /// `c.<x>.<z>.mcc` 边车文件与 Paper 超大边车文件会被合并回
    /// 依据 `path` 决定。
    fn read_at(r: Bytes, path: &Path) -> Result<Self, ChunkReadingError> {
        let mut file = Self::read(r)?;
        file.fill_external_payloads(path);
        file.apply_paper_oversized(path);
        Ok(file)
    }

    fn read(r: Bytes) -> Result<Self, ChunkReadingError> {
        let mut raw_file_bytes = r;

        // 小于 8 KiB 头的文件不可能包含区块数据；
        // 文件头必定是因崩溃而损坏。应将其视为空，而不是
        // 而让整个区域失败。
        if raw_file_bytes.is_empty() {
            return Ok(Self::default());
        }

        if raw_file_bytes.len() < SECTOR_BYTES * 2 {
            warn!(
                "Region 文件大小 {} 字节小于其 8 KiB 头部；按空文件处理",
                raw_file_bytes.len()
            );
            return Ok(Self::default());
        }

        let headers = raw_file_bytes.split_to(SECTOR_BYTES * 2);
        let (mut location_bytes, mut timestamp_bytes) = headers.split_at(SECTOR_BYTES);

        let mut chunk_file = Self::default();

        let mut last_offset = 2;
        for i in 0..CHUNK_COUNT {
            let timestamp = timestamp_bytes.get_u32();
            let location = location_bytes.get_u32();

            let mut sector_count = (location & 0xFF) as usize;
            let sector_offset = (location >> 8) as usize;

            // 如果扇区偏移量或数量为 0，则区块不存在（我们不应解析空区块）。
            // 扇区 1 是时间戳表，因此区块也不能从那里开始。
            if sector_offset < 2 || sector_count == 0 {
                continue;
            }

            // Spigot 扩展：该区块需要的扇区数超过一字节
            // count 字段所能表示的范围；真实扇区数由
            // 存储在区块偏移处的长度前缀。
            if sector_count == MAX_IN_FILE_SECTORS as usize {
                let length_offset = (sector_offset - 2) * SECTOR_BYTES;
                let Some(declared_length) = raw_file_bytes
                    .get(length_offset..length_offset + 4)
                    .and_then(|length| <[u8; 4]>::try_from(length).ok())
                    .map(u32::from_be_bytes)
                else {
                    warn!("Region 条目 {i} 标记为 255 个扇区但越界；丢弃该条目");
                    continue;
                };
                sector_count = (declared_length as usize + 4) / SECTOR_BYTES + 1;
            }

            let end_offset = sector_offset + sector_count;

            // 对最前面两个存放时间戳表和位置表的扇区，我们总是要减去 2
            // 我们之前遍历过的
            let bytes_offset = (sector_offset - 2) * SECTOR_BYTES;
            let bytes_count = sector_count * SECTOR_BYTES;

            if bytes_offset + bytes_count > raw_file_bytes.len() {
                // 头部自愈：损坏的条目不得拖垮整个
                // 区域文件；丢弃该条目并保留其余条目。
                warn!(
                    "Region 条目 {i}（扇区 {sector_offset}..{end_offset}）越界\
                     （{} 字节）；丢弃该条目",
                    raw_file_bytes.len()
                );
                continue;
            }

            let serialized_data = match AnvilChunkData::from_bytes(
                raw_file_bytes.slice(bytes_offset..bytes_offset + bytes_count),
            ) {
                Ok(data) => data,
                Err(err) => {
                    warn!("Region 条目 {i} 解析失败（{err:?}）；丢弃该条目");
                    continue;
                }
            };

            if end_offset > last_offset {
                last_offset = end_offset;
            }

            chunk_file.chunks_data[i] = Some(AnvilChunkMetadata {
                serialized_data,
                timestamp,
                file_sector_offset: sector_offset as u32,
            });
        }

        chunk_file.end_sector = last_offset as u32;
        Ok(chunk_file)
    }

    async fn update_chunk(
        &mut self,
        chunk: Arc<Self::Data>,
        chunk_config: &Self::ChunkConfig,
    ) -> Result<(), ChunkWritingError> {
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as u32;

        let index = Self::get_chunk_index(chunk.position().0, chunk.position().1);
        // 默认为从文件中读取的压缩类型
        let compression_type = self.chunks_data[index]
            .as_ref()
            .and_then(|chunk_data| chunk_data.serialized_data.compression);
        let chunk_config_snapshot = chunk_config.clone();
        let mut new_chunk_data = run_blocking(move || {
            AnvilChunkData::from_chunk(&*chunk, compression_type, &chunk_config_snapshot)
        })
        .await
        .map_err(|_| ChunkWritingError::IoError(std::io::Error::other("区块序列化任务失败")))??;

        // 负载超过 location 的 255 扇区上限的区块
        // 表会进入外部的 `c.<x>.<z>.mcc` 文件，与原版完全一致。
        // 这样一来，单字节扇区计数永远不会溢出。
        if new_chunk_data.is_oversized() {
            new_chunk_data.external = true;
        }

        let mut write_action = self.write_action.lock().await;
        if !chunk_config.write_in_place {
            *write_action = WriteAction::All;
        }

        match &*write_action {
            WriteAction::All => {
                trace!("写入动作为全部：原地设置区块");
                self.chunks_data[index] = Some(AnvilChunkMetadata {
                    serialized_data: new_chunk_data,
                    timestamp: epoch,
                    file_sector_offset: 0,
                });
                // 保持内存中的偏移与完整布局同步
                // 重写所产生的格式，因此后续的就地写入仍然有效。
                Self::renumber_sectors(&mut self.chunks_data, &mut self.end_sector);
            }
            _ => {
                match self.chunks_data[index].as_ref() {
                    None => {
                        trace!(
                            "区块 {} 不存在，追加到文件末尾: {}:{}",
                            index,
                            self.end_sector,
                            new_chunk_data.sector_count()
                        );
                        // 此区块此前不存在；追加到 EOF
                        let new_eof = self.end_sector + new_chunk_data.sector_count();
                        self.chunks_data[index] = Some(AnvilChunkMetadata {
                            serialized_data: new_chunk_data,
                            timestamp: epoch,
                            file_sector_offset: self.end_sector,
                        });
                        self.end_sector = new_eof;
                        write_action.maybe_update_chunk_index(index);
                    }
                    Some(old_chunk) => {
                        if old_chunk.serialized_data.sector_count() == new_chunk_data.sector_count()
                        {
                            trace!(
                                "区块 {} 已存在，原地写入: {}:{}",
                                index,
                                old_chunk.file_sector_offset,
                                new_chunk_data.sector_count()
                            );
                            // 我们可以直接添加它（这也涵盖了外部
                            // 区块：同尺寸意味着仅 `.mcc` 负载
                            // 以及头部字节变化）
                            self.chunks_data[index] = Some(AnvilChunkMetadata {
                                serialized_data: new_chunk_data,
                                timestamp: epoch,
                                file_sector_offset: old_chunk.file_sector_offset,
                            });
                            write_action.maybe_update_chunk_index(index);
                        } else {
                            // 区块大小发生了变化。正在重新整理其中的扇区
                            // 放置会留下一个窗口期，期间崩溃可能损坏
                            // 相邻区块，因此回退到原子的
                            // 改为整体重写。
                            trace!("区块 {} 大小发生变化，回退为原子化整体重写", index);
                            *write_action = WriteAction::All;
                            self.chunks_data[index] = Some(AnvilChunkMetadata {
                                serialized_data: new_chunk_data,
                                timestamp: epoch,
                                file_sector_offset: 0,
                            });
                            Self::renumber_sectors(&mut self.chunks_data, &mut self.end_sector);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn get_chunks(
        &self,
        chunks: Vec<Vector2<i32>>,
        stream: tokio::sync::mpsc::Sender<LoadedData<Self::Data, ChunkReadingError>>,
    ) {
        let chunk_items: Vec<(Vector2<i32>, Option<AnvilChunkData>)> = chunks
            .into_iter()
            .map(|chunk| {
                let index = Self::get_chunk_index(chunk.x, chunk.y);
                let data = self.chunks_data[index]
                    .as_ref()
                    .map(|chunk_metadata| chunk_metadata.serialized_data.clone());
                (chunk, data)
            })
            .collect();

        let (tx, mut rx) = tokio::sync::mpsc::channel(chunk_items.len().max(1));

        rayon::spawn(move || {
            use rayon::prelude::*;
            chunk_items
                .into_par_iter()
                .for_each(|(chunk, serialized_data)| {
                    let result = serialized_data.map_or_else(
                        || LoadedData::Missing(chunk),
                        |data| match data.to_chunk(chunk) {
                            Ok(chunk_res) => LoadedData::Loaded(chunk_res),
                            Err(err) => LoadedData::Error((chunk, err)),
                        },
                    );
                    let _ = tx.blocking_send(result);
                });
        });

        while let Some(item) = rx.recv().await {
            if stream.send(item).await.is_err() {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use papokin_config::chunk::ChunkCompression;
    use papokin_nbt::tag::NbtTag;

    fn config_with(write_in_place: bool) -> AnvilChunkConfig {
        AnvilChunkConfig {
            compression: ChunkCompression::default(),
            write_in_place,
        }
    }

    /// 一个用于序列化器级别测试的小型区块负载载体。它还会记录
    /// 哪些非负载键在往返后保留了下来，以观察合并情况。
    struct MockChunk {
        x: i32,
        z: i32,
        payload: Vec<u8>,
        extra_keys: Vec<String>,
        dirty: std::sync::atomic::AtomicBool,
    }

    impl Clone for MockChunk {
        fn clone(&self) -> Self {
            Self {
                x: self.x,
                z: self.z,
                payload: self.payload.clone(),
                extra_keys: self.extra_keys.clone(),
                dirty: std::sync::atomic::AtomicBool::new(
                    self.dirty.load(std::sync::atomic::Ordering::Relaxed),
                ),
            }
        }
    }

    impl std::fmt::Debug for MockChunk {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockChunk")
                .field("x", &self.x)
                .field("z", &self.z)
                .field("payload_len", &self.payload.len())
                .field("extra_keys", &self.extra_keys)
                .finish_non_exhaustive()
        }
    }

    impl MockChunk {
        fn new(x: i32, z: i32, payload: Vec<u8>) -> Arc<Self> {
            Arc::new(Self {
                x,
                z,
                payload,
                extra_keys: Vec::new(),
                dirty: std::sync::atomic::AtomicBool::new(true),
            })
        }
    }

    impl Dirtiable for MockChunk {
        fn is_dirty(&self) -> bool {
            self.dirty.load(std::sync::atomic::Ordering::Relaxed)
        }
        fn mark_dirty(&self, flag: bool) {
            self.dirty.store(flag, std::sync::atomic::Ordering::Relaxed);
        }
    }

    impl SingleChunkDataSerializer for MockChunk {
        fn to_bytes(&self) -> Result<Bytes, ChunkSerializingError> {
            let mut root = papokin_nbt::NbtCompound::new();
            root.put_int("x", self.x);
            root.put_int("z", self.z);
            // 拆分负载：`papokin-nbt` 拒绝读取更大的数组
            // 超过 `MAX_ARRAY_LENGTH`（512_000）个元素。
            for (part_index, part) in self.payload.chunks(500_000).enumerate() {
                root.put(
                    &format!("payload_{part_index}"),
                    NbtTag::ByteArray(part.iter().map(|&b| b as i8).collect()),
                );
            }
            Ok(papokin_nbt::Nbt::from(root).write_unnamed())
        }

        fn from_bytes(bytes: &Bytes, pos: Vector2<i32>) -> Result<Self, ChunkReadingError> {
            let mut cursor = std::io::Cursor::new(bytes.as_ref());
            let mut reader = papokin_nbt::deserializer::NbtReadHelperJava::new(
                papokin_nbt::deserializer::NbtStreamReader(&mut cursor),
            );
            let nbt = papokin_nbt::Nbt::read_unnamed(&mut reader).map_err(|e| {
                ChunkReadingError::ParsingError(ChunkParsingError::ErrorDeserializingChunk(
                    e.to_string(),
                ))
            })?;
            let mut parts: Vec<(usize, Vec<u8>)> = nbt
                .root_tag
                .child_tags
                .iter()
                .filter_map(|(name, tag)| {
                    let index = name.strip_prefix("payload_")?.parse().ok()?;
                    match tag {
                        NbtTag::ByteArray(arr) => {
                            Some((index, arr.iter().map(|&b| b as u8).collect()))
                        }
                        _ => None,
                    }
                })
                .collect();
            parts.sort_by_key(|(index, _)| *index);
            let payload = parts.into_iter().flat_map(|(_, part)| part).collect();
            let extra_keys = nbt
                .root_tag
                .child_tags
                .keys()
                .map(std::string::ToString::to_string)
                .filter(|key| key != "x" && key != "z" && !key.starts_with("payload_"))
                .collect();
            Ok(Self {
                x: pos.x,
                z: pos.y,
                payload,
                extra_keys,
                dirty: std::sync::atomic::AtomicBool::new(false),
            })
        }

        fn position(&self) -> (i32, i32) {
            (self.x, self.z)
        }
    }

    fn zlib_wrap(payload: &[u8]) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(payload, flate2::Compression::new(6));
        let mut out = Vec::new();
        encoder.read_to_end(&mut out).expect("zlib 编码失败");
        out
    }

    fn mock_nbt_bytes(payload: &[u8]) -> Vec<u8> {
        let mut root = papokin_nbt::NbtCompound::new();
        root.put_int("x", 0);
        root.put_int("z", 0);
        root.put(
            "payload_0",
            NbtTag::ByteArray(payload.iter().map(|&b| b as i8).collect()),
        );
        papokin_nbt::Nbt::from(root).write_unnamed().to_vec()
    }

    /// 构建原始区域文件：文件头 + 每个规格一个区块。
    /// `entries`：（索引、时间戳、按扇区对齐的记录，含 5 字节头）
    fn build_region(entries: &[(usize, u32, Vec<u8>)]) -> Vec<u8> {
        let mut location = vec![0u8; SECTOR_BYTES];
        let mut timestamps = vec![0u8; SECTOR_BYTES];
        let mut data = Vec::new();
        let mut next_sector: u32 = 2;

        for (index, timestamp, record) in entries {
            let count = record.len().div_ceil(SECTOR_BYTES) as u32;
            location[*index * 4..*index * 4 + 4]
                .copy_from_slice(&((next_sector << 8) | count).to_be_bytes());
            timestamps[*index * 4..*index * 4 + 4].copy_from_slice(&timestamp.to_be_bytes());
            data.extend_from_slice(record);
            data.extend(std::iter::repeat_n(
                0u8,
                count as usize * SECTOR_BYTES - record.len(),
            ));
            next_sector += count;
        }

        let mut file = Vec::new();
        file.extend_from_slice(&location);
        file.extend_from_slice(&timestamps);
        file.extend_from_slice(&data);
        file
    }

    fn chunk_record(compression: Option<u8>, payload: &[u8]) -> Vec<u8> {
        let mut record = Vec::with_capacity(payload.len() + 5);
        record.extend_from_slice(&((payload.len() + 1) as u32).to_be_bytes());
        record.push(compression.unwrap_or(Compression::NO_COMPRESSION_ID));
        record.extend_from_slice(payload);
        record
    }

    fn load_all(
        region: &AnvilChunkFile<MockChunk>,
        positions: Vec<Vector2<i32>>,
    ) -> Vec<LoadedData<MockChunk, ChunkReadingError>> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(positions.len().max(1));
        tokio::runtime::Runtime::new()
            .expect("运行时")
            .block_on(async move {
                region.get_chunks(positions, tx).await;
                let mut results = Vec::new();
                while let Some(item) = rx.recv().await {
                    results.push(item);
                }
                results
            })
    }

    #[test]
    fn read_accepts_spigot_255_extension() {
        // 区块所需的扇区数超出了单字节计数字段所能
        // 表示；计数字节在 255 处饱和，读取方必须
        // 从长度前缀重新计算真实数量。
        let payload = zlib_wrap(&mock_nbt_bytes(&vec![0xAB; 300 * 1024]));
        let record = chunk_record(Some(Compression::ZLib as u8), &payload);
        let mut file = build_region(&[(0, 42, record)]);
        // 对条目 0 的数量字节做饱和处理（偏移保持在扇区 2）。
        file[0..4].copy_from_slice(&((2u32 << 8) | 255).to_be_bytes());

        let region =
            <AnvilChunkFile<MockChunk> as ChunkSerializer>::read(file.into()).expect("解析成功");
        let results = load_all(&region, vec![Vector2::new(0, 0)]);
        match &results[0] {
            LoadedData::Loaded(chunk) => {
                assert_eq!(chunk.payload.len(), 300 * 1024);
                assert!(chunk.payload.iter().all(|&b| b == 0xAB));
            }
            other => panic!("预期为已加载区块，实际为 {other:?}"),
        }
    }

    #[test]
    fn read_heals_out_of_bounds_entries() {
        // 条目声明了远超文件大小的扇区：必须丢弃，不得
        // 而让整个区域失败，且第二个条目必须仍在原处。
        let good_payload = zlib_wrap(&mock_nbt_bytes(b"good"));
        let good = chunk_record(Some(Compression::ZLib as u8), &good_payload);
        let mut file = build_region(&[(5, 1, good)]);
        // 条目 0 指向文件之外
        file[0..4].copy_from_slice(&((9000u32 << 8) | 3).to_be_bytes());

        let region =
            <AnvilChunkFile<MockChunk> as ChunkSerializer>::read(file.into()).expect("解析成功");
        let results = load_all(&region, vec![Vector2::new(5, 0), Vector2::new(0, 0)]);
        assert!(
            results
                .iter()
                .any(|item| matches!(item, LoadedData::Missing(pos) if pos.x == 0))
        );
        assert!(
            results
                .iter()
                .any(|item| matches!(item, LoadedData::Loaded(c) if c.payload == b"good"))
        );
    }

    #[test]
    fn torn_header_is_treated_as_empty() {
        let file = vec![0u8; 100];
        let region =
            <AnvilChunkFile<MockChunk> as ChunkSerializer>::read(file.into()).expect("解析成功");
        let results = load_all(&region, vec![Vector2::new(0, 0)]);
        assert!(matches!(results[0], LoadedData::Missing(_)));
    }

    #[test]
    fn external_entry_with_missing_mcc_is_per_chunk_error() {
        let temp = tempfile::tempdir().expect("临时目录");
        let region_path = temp.path().join("r.0.0.mca");

        // 条目 0 设置了外部标志，但旁边没有 `.mcc` 文件
        let header = chunk_record(Some(Compression::ZLib as u8 | 0x80), b"");
        let file = build_region(&[(0, 7, header)]);

        let region =
            <AnvilChunkFile<MockChunk> as ChunkSerializer>::read_at(file.into(), &region_path)
                .expect("解析成功");
        let results = load_all(&region, vec![Vector2::new(0, 0)]);
        assert!(matches!(results[0], LoadedData::Error(_)));
    }

    #[test]
    fn paper_oversized_data_is_merged_on_read() {
        let temp = tempfile::tempdir().expect("临时目录");
        let region_path = temp.path().join("r.0.0.mca");

        // 基础区块 0/0，不含实体，以 zlib 内联存储。
        let base_payload = zlib_wrap(&mock_nbt_bytes(b"base"));
        let record = chunk_record(Some(Compression::ZLib as u8), &base_payload);
        let file = build_region(&[(0, 1, record)]);
        std::fs::write(&region_path, &file).expect("写入区域");

        // 存放提取出的实体数据的超大附属文件。
        let mut root = papokin_nbt::NbtCompound::new();
        let mut entity = papokin_nbt::NbtCompound::new();
        entity.put_string("id", "minecraft:zombie".to_string());
        root.put_list("Entities", vec![NbtTag::Compound(entity)]);
        let sidecar = papokin_nbt::Nbt::new(String::new(), root);
        let sidecar_path = temp.path().join("r.0.0_oversized_0_0.nbt");
        std::fs::write(&sidecar_path, zlib_wrap(&sidecar.write())).expect("写入 sidecar");

        // 将区块 0 标记为超大区块的位图。
        let mut meta = vec![0u8; CHUNK_COUNT];
        meta[0] = 1;
        std::fs::write(temp.path().join("r.0.0.oversized.nbt"), meta).expect("写入元数据");

        let region =
            <AnvilChunkFile<MockChunk> as ChunkSerializer>::read_at(file.into(), &region_path)
                .expect("解析成功");
        let results = load_all(&region, vec![Vector2::new(0, 0)]);
        match &results[0] {
            LoadedData::Loaded(chunk) => {
                assert_eq!(chunk.payload, b"base");
                assert!(chunk.extra_keys.iter().any(|key| key == "Entities"));
            }
            other => panic!("预期为已加载区块，实际为 {other:?}"),
        }
    }

    #[tokio::test]
    async fn oversized_chunk_written_as_external_never_overflows_location_table() {
        let temp = tempfile::tempdir().expect("临时目录");
        let region_path = temp.path().join("r.0.0.mca");

        // 负载大到需要超过 255 个扇区（约 1 MiB）
        // 压缩：不可压缩的 xorshift 字节，而非重复模式。
        let mut payload = Vec::with_capacity(1200 * 1024);
        let mut state: u64 = 0x0051_7CC1_B7C7_1F15;
        for _ in 0..1200 * 1024 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            payload.push(state as u8);
        }
        let chunk = MockChunk::new(3, 4, payload.clone());
        let mut region: AnvilChunkFile<MockChunk> = AnvilChunkFile::default();
        region
            .update_chunk(chunk, &config_with(true))
            .await
            .expect("更新成功");

        region.write(&region_path).await.expect("写入成功");

        // 位置条目必须保持可表示：count == 1 个扇区，并且
        // 该偏移处的头字节必须带有外部标志。
        let bytes = tokio::fs::read(&region_path).await.expect("回读");
        let entry_index = AnvilChunkFile::<MockChunk>::get_chunk_index(3, 4);
        let entry_slice = &bytes[entry_index * 4..entry_index * 4 + 4];
        let entry = u32::from_be_bytes(entry_slice.try_into().expect("条目 4 字节"));
        let (offset, count) = (entry >> 8, entry & 0xFF);
        assert_eq!(count, 1, "external chunks occupy a single header sector");
        let header_byte = bytes[offset as usize * SECTOR_BYTES + 4];
        assert_ne!(
            header_byte & EXTERNAL_FLAG,
            0,
            "header byte must carry the external flag"
        );
        assert!(Compression::from_byte(header_byte & !EXTERNAL_FLAG).is_ok());

        // 外部负载文件必须存在。
        assert!(temp.path().join("c.3.4.mcc").exists());

        // 往返测试：read_at 必须从外部文件加载区块。
        let region =
            <AnvilChunkFile<MockChunk> as ChunkSerializer>::read_at(bytes.into(), &region_path)
                .expect("解析");
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        region.get_chunks(vec![Vector2::new(3, 4)], tx).await;
        while let Some(item) = rx.recv().await {
            match item {
                LoadedData::Loaded(chunk) => assert_eq!(chunk.payload, payload),
                other => panic!("预期为已加载区块，实际为 {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn atomic_rewrite_round_trip() {
        let temp = tempfile::tempdir().expect("临时目录");
        let region_path = temp.path().join("r.0.0.mca");

        let mut region: AnvilChunkFile<MockChunk> = AnvilChunkFile::default();
        region
            .update_chunk(MockChunk::new(0, 0, b"hello".to_vec()), &config_with(false))
            .await
            .expect("更新 1");
        region
            .update_chunk(
                MockChunk::new(31, 31, b"world".to_vec()),
                &config_with(false),
            )
            .await
            .expect("更新 2");
        region.write(&region_path).await.expect("写入");

        let raw = tokio::fs::read(&region_path).await.expect("读取");
        let region =
            <AnvilChunkFile<MockChunk> as ChunkSerializer>::read_at(raw.into(), &region_path)
                .expect("解析");
        let (tx, mut rx) = tokio::sync::mpsc::channel(2);
        region
            .get_chunks(vec![Vector2::new(0, 0), Vector2::new(31, 31)], tx)
            .await;
        let mut loaded = 0;
        while let Some(item) = rx.recv().await {
            if let LoadedData::Loaded(chunk) = item {
                assert!(chunk.payload == b"hello" || chunk.payload == b"world");
                loaded += 1;
            }
        }
        assert_eq!(loaded, 2);

        // 无残留的临时文件。
        let mut entries = tokio::fs::read_dir(temp.path()).await.expect("读取目录");
        while let Some(entry) = entries.next_entry().await.expect("条目") {
            let name = entry.file_name().to_string_lossy().to_string();
            let extension = std::path::Path::new(&name)
                .extension()
                .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            assert!(
                extension != "tmp" && extension != "tmp_atomic",
                "leftover temp file {name}"
            );
        }
    }

    #[tokio::test]
    async fn in_place_write_survives_full_rewrite_followup() {
        // 针对过期偏移量隐患的回归测试：在原子化完全重写之后
        // （全部），后续同尺寸的原地写入仍必须落在
        // 正确的扇区，而非旧文件布局。
        let temp = tempfile::tempdir().expect("临时目录");
        let region_path = temp.path().join("r.0.0.mca");

        let mut region: AnvilChunkFile<MockChunk> = AnvilChunkFile::default();
        // 首次保存：默认（原子 All）路径
        region
            .update_chunk(MockChunk::new(2, 2, b"first".to_vec()), &config_with(false))
            .await
            .expect("更新 1");
        region.write(&region_path).await.expect("写入 1");

        // 第二次保存：原地模式，相同大小的负载。
        region
            .update_chunk(MockChunk::new(2, 2, b"secnd".to_vec()), &config_with(true))
            .await
            .expect("更新 2");
        region.write(&region_path).await.expect("写入 2");

        let raw = tokio::fs::read(&region_path).await.expect("读取");
        let region =
            <AnvilChunkFile<MockChunk> as ChunkSerializer>::read_at(raw.into(), &region_path)
                .expect("解析");
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        region.get_chunks(vec![Vector2::new(2, 2)], tx).await;
        while let Some(item) = rx.recv().await {
            match item {
                LoadedData::Loaded(chunk) => assert_eq!(chunk.payload, b"secnd"),
                other => panic!("预期为已加载区块，实际为 {other:?}"),
            }
        }
    }

    #[test]
    fn compression_round_trip_all_variants() {
        for compression in [Compression::GZip, Compression::ZLib, Compression::LZ4] {
            let data = b"the quick brown fox jumps over the lazy dog".repeat(64);
            let compressed = compression.compress_data(&data, 6).expect("压缩失败");
            let decompressed = compression.decompress_data(&compressed).expect("解压失败");
            assert_eq!(decompressed.as_ref(), data.as_slice(), "{compression:?}");
        }
    }
}
