use rustc_hash::FxHashMap;
use std::io::{ErrorKind, Read};
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::chunk::format::anvil::{AnvilChunkFile, SingleChunkDataSerializer};
use crate::chunk::io::{ChunkSerializer, LoadedData, run_blocking};
use crate::chunk::{ChunkReadingError, ChunkWritingError};
use bytes::{Buf, BufMut, Bytes};
use papokin_util::math::vector2::Vector2;
use ruzstd::decoding::StreamingDecoder;
use ruzstd::encoding::{CompressionLevel, compress_to_vec};
use tokio::io::{AsyncWriteExt, BufWriter};
use tracing::{error, warn};
use xxhash_rust::xxh64::xxh64;

use super::anvil::CHUNK_COUNT;

/// 同时用作页眉和页脚的签名。
/// <https://gist.github.com/Aaron2550/5701519671253d4c6190bde6706f9f98>
const SIGNATURE: [u8; 8] = u64::to_be_bytes(0xc3ff13183cca9d9a);

/// v2 桶细分允许的网格大小。
const VALID_GRID_SIZES: &[u8] = &[1, 2, 4, 8, 16, 32];

/// 默认网格大小。2×2 = 4 个桶可提供合理的写放大
/// 对大多数世界来说是不错的折中。
const DEFAULT_GRID_SIZE: u8 = 2;

// ---------------------------------------------------------------------------
// 超级块（26 字节）
// ---------------------------------------------------------------------------
//
//  0.. 8   uint64  签名（0xc3ff13183cca9d9a）
//  8.. 9   uint8   版本（2）
//  9..17   uint64  最新时间戳
// 17..18   int8    网格尺寸
// 18..22   int32   区域 X
// 22..26   int32   区域 Z

struct LinearV2Superblock {
    newest_timestamp: u64,
    /// {1, 2, 4, 8, 16, 32} 之一。
    grid_size: u8,
    region_x: i32,
    region_z: i32,
}

impl LinearV2Superblock {
    const SIZE: usize = 26;

    fn from_bytes(buf: &[u8]) -> Result<Self, ChunkReadingError> {
        if buf.len() < Self::SIZE {
            return Err(ChunkReadingError::IoError(std::io::Error::from(
                ErrorKind::UnexpectedEof,
            )));
        }
        let mut b = buf;
        let mut sig = [0u8; 8];
        sig.copy_from_slice(&b[..8]);
        b = &b[8..];

        if sig != SIGNATURE {
            error!("Linear v2: 超级块签名无效");
            return Err(ChunkReadingError::InvalidHeader);
        }

        let version = b.get_u8();
        if version != 0x02 {
            error!("Linear v2: 超级块中的版本字节 {version:#x} 不符合预期");
            return Err(ChunkReadingError::InvalidHeader);
        }

        let newest_timestamp = b.get_u64();
        let grid_size = b.get_u8();
        let region_x = b.get_i32();
        let region_z = b.get_i32();

        if !VALID_GRID_SIZES.contains(&grid_size) {
            error!("Linear v2: 网格大小 {grid_size} 无效");
            return Err(ChunkReadingError::InvalidHeader);
        }

        Ok(Self {
            newest_timestamp,
            grid_size,
            region_x,
            region_z,
        })
    }

    fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut out = [0u8; Self::SIZE];
        let mut b = out.as_mut_slice();
        b.put_slice(&SIGNATURE);
        b.put_u8(0x02);
        b.put_u64(self.newest_timestamp);
        b.put_u8(self.grid_size);
        b.put_i32(self.region_x);
        b.put_i32(self.region_z);
        out
    }
}

// ---------------------------------------------------------------------------
// 区块存在位图  (128 字节 = 1024 位)
// ---------------------------------------------------------------------------

struct ChunkBitmap([u8; 128]);

impl ChunkBitmap {
    const SIZE: usize = 128;

    const fn new() -> Self {
        Self([0u8; 128])
    }

    const fn set(&mut self, index: usize, exists: bool) {
        let byte = index / 8;
        let bit = index % 8;
        if exists {
            self.0[byte] |= 1 << bit;
        } else {
            self.0[byte] &= !(1 << bit);
        }
    }

    #[allow(dead_code)]
    const fn get(&self, index: usize) -> bool {
        let byte = index / 8;
        let bit = index % 8;
        (self.0[byte] >> bit) & 1 == 1
    }
}

// ---------------------------------------------------------------------------
// NBT 特性字典
// ---------------------------------------------------------------------------
//
// 重复：u8 key_len、键字节、u32 value
// 以单个 0x00 字节终止。

struct NbtFeatures(FxHashMap<String, u32>);

impl NbtFeatures {
    fn empty() -> Self {
        Self(FxHashMap::default())
    }

    fn from_bytes(buf: &mut impl Buf) -> Result<Self, ChunkReadingError> {
        let mut map = FxHashMap::default();
        loop {
            if !buf.has_remaining() {
                return Err(ChunkReadingError::IoError(std::io::Error::from(
                    ErrorKind::UnexpectedEof,
                )));
            }
            let key_len = buf.get_u8();
            if key_len == 0 {
                break;
            }
            if buf.remaining() < key_len as usize + 4 {
                return Err(ChunkReadingError::IoError(std::io::Error::from(
                    ErrorKind::UnexpectedEof,
                )));
            }
            let key_bytes: Vec<u8> = (0..key_len).map(|_| buf.get_u8()).collect();
            let key = String::from_utf8(key_bytes).map_err(|_| ChunkReadingError::InvalidHeader)?;
            let value = buf.get_u32();
            map.insert(key, value);
        }
        Ok(Self(map))
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for (key, value) in &self.0 {
            let key_bytes = key.as_bytes();
            out.push(key_bytes.len() as u8);
            out.extend_from_slice(key_bytes);
            out.extend_from_slice(&value.to_be_bytes());
        }
        out.push(0); // 结束标记
        out
    }
}

// ---------------------------------------------------------------------------
// 桶大小条目  (每桶 13 字节)
// ---------------------------------------------------------------------------
//
//  0..4    uint32   桶大小（压缩后）
//  4..5    int8     压缩级别
//  5..13   uint64   压缩桶数据的 xxhash64

struct BucketSizeEntry {
    size: u32,
    compression_level: i8,
    xxhash: u64,
}

impl BucketSizeEntry {
    const SIZE: usize = 13;

    fn from_bytes(buf: &mut impl Buf) -> Result<Self, ChunkReadingError> {
        if buf.remaining() < Self::SIZE {
            return Err(ChunkReadingError::IoError(std::io::Error::from(
                ErrorKind::UnexpectedEof,
            )));
        }
        Ok(Self {
            size: buf.get_u32(),
            compression_level: buf.get_i8(),
            xxhash: buf.get_u64(),
        })
    }

    fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut out = [0u8; Self::SIZE];
        let mut b = out.as_mut_slice();
        b.put_u32(self.size);
        b.put_i8(self.compression_level);
        b.put_u64(self.xxhash);
        out
    }
}

// ---------------------------------------------------------------------------
// 解压存储桶内的每区块记录
// ---------------------------------------------------------------------------
//
//  0.. 4   uint32   区块大小（0 = 不存在）
//  4..12   uint64   区块时间戳
// 12..     字节    区块 NBT 数据（仅当 size > 0 时）

struct BucketChunkEntry {
    timestamp: u64,
    data: Option<Bytes>,
}

impl BucketChunkEntry {
    /// 固定部分（大小 + 时间戳）的字节数。
    const FIXED_SIZE: usize = 12;

    fn write_into(&self, out: &mut Vec<u8>) {
        match &self.data {
            None => {
                out.put_u32(0);
                out.put_u64(self.timestamp);
            }
            Some(bytes) => {
                out.put_u32(bytes.len() as u32);
                out.put_u64(self.timestamp);
                out.extend_from_slice(bytes);
            }
        }
    }

    fn read_from(buf: &mut Bytes) -> Result<Self, ChunkReadingError> {
        if buf.remaining() < Self::FIXED_SIZE {
            return Err(ChunkReadingError::IoError(std::io::Error::from(
                ErrorKind::UnexpectedEof,
            )));
        }
        let size = buf.get_u32() as usize;
        let timestamp = buf.get_u64();
        if size == 0 {
            return Ok(Self {
                timestamp,
                data: None,
            });
        }
        if buf.remaining() < size {
            warn!(
                "Linear v2: 区块数据字节不足（需要 {size}，实际 {}）",
                buf.remaining()
            );
            return Err(ChunkReadingError::IoError(std::io::Error::from(
                ErrorKind::UnexpectedEof,
            )));
        }
        let data = buf.split_to(size);
        Ok(Self {
            timestamp,
            data: Some(data),
        })
    }
}

// ---------------------------------------------------------------------------
// 主 LinearV2File 结构体
// ---------------------------------------------------------------------------

pub struct LinearV2File<S: SingleChunkDataSerializer> {
    /// 区域坐标，写入时重建超级块时需要。
    region_x: i32,
    region_z: i32,

    /// 网格大小（1、2、4、8、16 或 32）。
    grid_size: u8,

    /// 每个区块的时间戳。
    timestamps: [u64; CHUNK_COUNT],

    /// 每个区块的压缩 NBT 数据（None 表示区块不存在）。
    chunks_data: [Option<Bytes>; CHUNK_COUNT],

    _dummy: PhantomData<S>,
}

#[expect(clippy::large_stack_arrays)]
impl<S: SingleChunkDataSerializer> Default for LinearV2File<S> {
    fn default() -> Self {
        Self {
            region_x: 0,
            region_z: 0,
            grid_size: DEFAULT_GRID_SIZE,
            timestamps: [0u64; CHUNK_COUNT],
            chunks_data: [const { None }; CHUNK_COUNT],
            _dummy: PhantomData,
        }
    }
}

impl<S: SingleChunkDataSerializer> LinearV2File<S> {
    const fn get_chunk_index(x: i32, z: i32) -> usize {
        AnvilChunkFile::<S>::get_chunk_index(x, z)
    }

    /// 网格中的桶数量（`grid_size²`）。
    const fn bucket_count(grid_size: u8) -> usize {
        (grid_size as usize) * (grid_size as usize)
    }

    /// 每个桶中的区块数量。
    const fn chunks_per_bucket(grid_size: u8) -> usize {
        CHUNK_COUNT / Self::bucket_count(grid_size)
    }

    /// 为桶 `bucket_idx` 构建解压后的字节流。
    fn serialise_bucket(
        chunks_data: &[Option<Bytes>; CHUNK_COUNT],
        timestamps: &[u64; CHUNK_COUNT],
        bucket_idx: usize,
        grid_size: u8,
    ) -> Vec<u8> {
        let cpb = Self::chunks_per_bucket(grid_size);
        let mut buf = Vec::new();

        for local in 0..cpb {
            // 从桶索引 + 局部索引恢复全局区块索引。
            let chunk_index = Self::global_chunk_index(bucket_idx, local, grid_size);
            let entry = BucketChunkEntry {
                timestamp: timestamps[chunk_index],
                data: chunks_data[chunk_index].clone(),
            };
            entry.write_into(&mut buf);
        }
        buf
    }

    /// `chunk_bucket_index` + `chunk_local_index` 的逆运算。
    const fn global_chunk_index(bucket_idx: usize, local: usize, grid_size: u8) -> usize {
        let stride = 32 / grid_size as usize;
        let bucket_row = bucket_idx / grid_size as usize;
        let bucket_col = bucket_idx % grid_size as usize;
        let local_row = local / stride;
        let local_col = local % stride;
        let cz = bucket_row * stride + local_row;
        let cx = bucket_col * stride + local_col;
        cz * 32 + cx
    }
}

impl<S: SingleChunkDataSerializer + 'static> ChunkSerializer for LinearV2File<S> {
    type Data = S;
    type WriteBackend = PathBuf;
    type ChunkConfig = ();

    fn should_write(&self, is_watched: bool) -> bool {
        !is_watched
    }

    fn get_chunk_key(chunk: &Vector2<i32>) -> String {
        let (region_x, region_z) = AnvilChunkFile::<S>::get_region_coords(chunk);
        format!("./r.{region_x}.{region_z}.linear")
    }

    async fn write(&self, path: &PathBuf) -> Result<(), std::io::Error> {
        let temp_path = path.with_extension("tmp");
        let grid_size = self.grid_size;
        let chunks_data = self.chunks_data.clone();
        let timestamps = self.timestamps;
        let region_x = self.region_x;
        let region_z = self.region_z;
        let (header, compressed_buckets) = Box::pin(run_blocking(move || {
            let bucket_count = Self::bucket_count(grid_size);
            let mut compressed_buckets: Vec<Box<[u8]>> = Vec::with_capacity(bucket_count);
            let mut bucket_entries: Vec<BucketSizeEntry> = Vec::with_capacity(bucket_count);

            for bucket_idx in 0..bucket_count {
                let raw = Self::serialise_bucket(&chunks_data, &timestamps, bucket_idx, grid_size);
                let compressed =
                    compress_to_vec(raw.as_slice(), CompressionLevel::Fastest).into_boxed_slice();
                bucket_entries.push(BucketSizeEntry {
                    size: compressed.len() as u32,
                    compression_level: 1,
                    xxhash: xxh64(&compressed, 0),
                });
                compressed_buckets.push(compressed);
            }

            let superblock = LinearV2Superblock {
                newest_timestamp: timestamps.iter().copied().max().unwrap_or(0),
                grid_size,
                region_x,
                region_z,
            };
            let mut bitmap = ChunkBitmap::new();
            for (index, chunk) in chunks_data.iter().enumerate() {
                if chunk.is_some() {
                    bitmap.set(index, true);
                }
            }
            let features = NbtFeatures::empty();

            let mut header = Vec::new();
            header.extend_from_slice(&superblock.to_bytes());
            header.extend_from_slice(&bitmap.0);
            header.extend_from_slice(&features.to_bytes());
            for entry in bucket_entries {
                header.extend_from_slice(&entry.to_bytes());
            }
            (header, compressed_buckets)
        }))
        .await
        .map_err(|_| std::io::Error::other("linear serialization task failed"))?;

        let file = tokio::fs::File::create(&temp_path).await?;
        let mut writer = BufWriter::new(file);
        writer.write_all(&header).await?;
        for compressed in compressed_buckets {
            writer.write_all(&compressed).await?;
        }
        writer.write_all(&SIGNATURE).await?;
        writer.flush().await?;

        // 原子重命名，使写入过程中崩溃不会产生残缺文件。
        tokio::fs::rename(temp_path, path).await?;
        Ok(())
    }

    #[expect(clippy::large_stack_arrays)]
    fn read(raw_file: Bytes) -> Result<Self, ChunkReadingError> {
        let mut buf = raw_file;

        if buf.remaining() < LinearV2Superblock::SIZE {
            return Err(ChunkReadingError::IoError(std::io::Error::from(
                ErrorKind::UnexpectedEof,
            )));
        }
        let superblock_bytes = buf.split_to(LinearV2Superblock::SIZE);
        let superblock = LinearV2Superblock::from_bytes(&superblock_bytes)?;
        let grid_size = superblock.grid_size;
        let bucket_count = Self::bucket_count(grid_size);

        if buf.remaining() < ChunkBitmap::SIZE {
            return Err(ChunkReadingError::IoError(std::io::Error::from(
                ErrorKind::UnexpectedEof,
            )));
        }
        // 我们读取了位图，但根据规范它尚不可靠，因此我们
        // 不要用它做短路判断 —— 实际存在性由
        // 每个桶内部的每区块大小字段。
        let mut bitmap_raw = [0u8; ChunkBitmap::SIZE];
        bitmap_raw.copy_from_slice(&buf.split_to(ChunkBitmap::SIZE));
        //let _bitmap = ChunkBitmap(bitmap_raw);

        let _features = NbtFeatures::from_bytes(&mut buf)?;

        let total_bucket_meta = bucket_count * BucketSizeEntry::SIZE;
        if buf.remaining() < total_bucket_meta {
            return Err(ChunkReadingError::IoError(std::io::Error::from(
                ErrorKind::UnexpectedEof,
            )));
        }
        let mut bucket_entries: Vec<BucketSizeEntry> = Vec::with_capacity(bucket_count);
        for _ in 0..bucket_count {
            bucket_entries.push(BucketSizeEntry::from_bytes(&mut buf)?);
        }

        // 算出页脚从哪里开始，以便我们无需
        // 尚未消耗桶数据字节。
        let total_compressed: usize = bucket_entries.iter().map(|e| e.size as usize).sum();
        if buf.remaining() < total_compressed + SIGNATURE.len() {
            return Err(ChunkReadingError::IoError(std::io::Error::from(
                ErrorKind::UnexpectedEof,
            )));
        }
        let footer_offset = total_compressed;
        {
            let footer = &buf[footer_offset..footer_offset + SIGNATURE.len()];
            if footer != SIGNATURE {
                error!("Linear v2: 尾部签名无效");
                return Err(ChunkReadingError::InvalidHeader);
            }
        }

        let mut timestamps = [0u64; CHUNK_COUNT];
        let mut chunks_data: [Option<Bytes>; CHUNK_COUNT] = [const { None }; CHUNK_COUNT];

        for (bucket_idx, entry) in bucket_entries.iter().enumerate() {
            let compressed_size = entry.size as usize;

            // xxhash64 完整性校验。
            let actual_hash = xxh64(&buf[..compressed_size], 0);
            if actual_hash != entry.xxhash {
                error!(
                    "Linear v2: 桶 {bucket_idx} 的 xxhash 不匹配\
                     （预期 {:#x}，实际 {:#x}）",
                    entry.xxhash, actual_hash
                );
                // 跳过损坏的桶，而不是中止整个
                // 文件——其余的桶可能仍然完好。
                buf.advance(compressed_size);
                continue;
            }

            let compressed_slice = &buf[..compressed_size];
            let mut decompressed = Vec::new();
            {
                let mut decoder = StreamingDecoder::new(compressed_slice)
                    .map_err(|_| ChunkReadingError::RegionIsInvalid)?;
                decoder
                    .read_to_end(&mut decompressed)
                    .map_err(ChunkReadingError::IoError)?
            };
            buf.advance(compressed_size);

            let mut bucket_buf: Bytes = decompressed.into();
            let cpb = Self::chunks_per_bucket(grid_size);

            for local in 0..cpb {
                let chunk_index = Self::global_chunk_index(bucket_idx, local, grid_size);
                let record = BucketChunkEntry::read_from(&mut bucket_buf)?;
                timestamps[chunk_index] = record.timestamp;
                chunks_data[chunk_index] = record.data;
            }
        }

        Ok(Self {
            region_x: superblock.region_x,
            region_z: superblock.region_z,
            grid_size,
            timestamps,
            chunks_data,
            _dummy: PhantomData,
        })
    }

    async fn update_chunk(
        &mut self,
        chunk: Arc<Self::Data>,
        _chunk_config: &Self::ChunkConfig,
    ) -> Result<(), ChunkWritingError> {
        let index = Self::get_chunk_index(chunk.position().0, chunk.position().1);
        let chunk_raw = run_blocking(move || {
            chunk
                .to_bytes()
                .map_err(|err| ChunkWritingError::ChunkSerializingError(err.to_string()))
        })
        .await
        .map_err(|_| {
            ChunkWritingError::IoError(std::io::Error::other("chunk serialization task failed"))
        })??;

        self.timestamps[index] = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        self.chunks_data[index] = Some(chunk_raw);
        Ok(())
    }

    async fn get_chunks(
        &self,
        chunks: Vec<Vector2<i32>>,
        stream: tokio::sync::mpsc::Sender<LoadedData<Self::Data, ChunkReadingError>>,
    ) {
        let chunk_items: Vec<(Vector2<i32>, Option<Bytes>)> = chunks
            .into_iter()
            .map(|chunk| {
                let index = Self::get_chunk_index(chunk.x, chunk.y);
                let data = self.chunks_data[index].clone();
                (chunk, data)
            })
            .collect();

        let (tx, mut rx) = tokio::sync::mpsc::channel(chunk_items.len().max(1));

        rayon::spawn(move || {
            use rayon::prelude::*;
            chunk_items.into_par_iter().for_each(|(chunk, data)| {
                let result = data.map_or_else(
                    || LoadedData::Missing(chunk),
                    |data| match S::from_bytes(&data, chunk) {
                        Ok(c) => LoadedData::Loaded(c),
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

    // 辅助函数：构建极小的区块负载。
    fn fake_nbt(seed: u8) -> Bytes {
        Bytes::from(vec![seed; 64])
    }

    #[test]
    fn bitmap_round_trip() {
        let mut bm = ChunkBitmap::new();
        bm.set(0, true);
        bm.set(7, true);
        bm.set(1023, true);
        assert!(bm.get(0));
        assert!(!bm.get(1));
        assert!(bm.get(7));
        assert!(!bm.get(8));
        assert!(bm.get(1023));
        assert!(!bm.get(1022));
    }

    #[test]
    fn nbt_features_empty_round_trip() {
        let f = NbtFeatures::empty();
        let encoded = f.to_bytes();
        assert_eq!(encoded, vec![0u8]); // 仅为结束标记

        let mut buf: Bytes = encoded.into();
        let decoded = NbtFeatures::from_bytes(&mut buf).unwrap();
        assert!(decoded.0.is_empty());
    }

    #[test]
    fn nbt_features_round_trip() {
        let mut f = NbtFeatures::empty();
        f.0.insert("version".into(), 42);
        let encoded = f.to_bytes();
        let mut buf: Bytes = encoded.into();
        let decoded = NbtFeatures::from_bytes(&mut buf).unwrap();
        assert_eq!(decoded.0.get("version"), Some(&42u32));
    }

    fn chunk_bucket_index(i: usize, gs: u8) -> usize {
        let cx = i % 32;
        let cz = i / 32;
        let stride = 32 / gs as usize;
        (cz / stride) * gs as usize + (cx / stride)
    }

    fn chunk_local_index(i: usize, gs: u8) -> usize {
        let cx = i % 32;
        let cz = i / 32;
        let stride = 32 / gs as usize;
        (cz % stride) * stride + (cx % stride)
    }

    fn global_chunk_index(bucket: usize, local: usize, gs: u8) -> usize {
        let stride = 32 / gs as usize;
        let brow = bucket / gs as usize;
        let bcol = bucket % gs as usize;
        let lrow = local / stride;
        let lcol = local % stride;
        let cz = brow * stride + lrow;
        let cx = bcol * stride + lcol;
        cz * 32 + cx
    }

    #[test]
    fn index_round_trip_grid2() {
        for i in 0..CHUNK_COUNT {
            let b = chunk_bucket_index(i, 2);
            let l = chunk_local_index(i, 2);
            let recovered = global_chunk_index(b, l, 2);
            assert_eq!(i, recovered, "chunk index {i} round-trip failed");
        }
    }

    #[test]
    fn index_round_trip_grid4() {
        for i in 0..CHUNK_COUNT {
            let b = chunk_bucket_index(i, 4);
            let l = chunk_local_index(i, 4);
            let recovered = global_chunk_index(b, l, 4);
            assert_eq!(i, recovered, "chunk index {i} round-trip failed");
        }
    }

    #[test]
    fn index_round_trip_grid1() {
        for i in 0..CHUNK_COUNT {
            let b = chunk_bucket_index(i, 1);
            let l = chunk_local_index(i, 1);
            let recovered = global_chunk_index(b, l, 1);
            assert_eq!(i, recovered, "chunk index {i} round-trip failed");
        }
    }

    #[test]
    fn superblock_round_trip() {
        let sb = LinearV2Superblock {
            newest_timestamp: 0xDEADBEEFCAFEBABE,
            grid_size: 4,
            region_x: -7,
            region_z: 13,
        };
        let encoded = sb.to_bytes();
        let decoded = LinearV2Superblock::from_bytes(&encoded).unwrap();
        assert_eq!(decoded.newest_timestamp, sb.newest_timestamp);
        assert_eq!(decoded.grid_size, 4);
        assert_eq!(decoded.region_x, -7);
        assert_eq!(decoded.region_z, 13);
    }

    #[test]
    fn superblock_rejects_bad_signature() {
        let mut bytes = LinearV2Superblock {
            newest_timestamp: 0,
            grid_size: 2,
            region_x: 0,
            region_z: 0,
        }
        .to_bytes();
        bytes[0] ^= 0xFF; // 破坏签名
        assert!(LinearV2Superblock::from_bytes(&bytes).is_err());
    }

    #[test]
    fn bucket_entry_present_round_trip() {
        let entry = BucketChunkEntry {
            timestamp: 12345,
            data: Some(fake_nbt(0xAB)),
        };
        let mut buf = Vec::new();
        entry.write_into(&mut buf);
        let mut b: Bytes = buf.into();
        let decoded = BucketChunkEntry::read_from(&mut b).unwrap();
        assert_eq!(decoded.timestamp, 12345);
        assert_eq!(decoded.data.unwrap().as_ref(), &[0xABu8; 64][..]);
    }

    #[test]
    fn bucket_entry_absent_round_trip() {
        let entry = BucketChunkEntry {
            timestamp: 0,
            data: None,
        };
        let mut buf = Vec::new();
        entry.write_into(&mut buf);
        let mut b: Bytes = buf.into();
        let decoded = BucketChunkEntry::read_from(&mut b).unwrap();
        assert!(decoded.data.is_none());
    }
}
