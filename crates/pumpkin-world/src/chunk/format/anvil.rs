use bytes::{Buf, BufMut, Bytes};
use flate2::read::{GzDecoder, GzEncoder, ZlibDecoder, ZlibEncoder};
use lz4_java_wrc::Context;
use pumpkin_config::chunk::AnvilChunkConfig;
use pumpkin_util::math::vector2::Vector2;
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
    io::{ChunkSerializer, Dirtiable, LoadedData, run_blocking},
};

/// The side size of a region in chunks (one region is 32x32 chunks)
pub const REGION_SIZE: usize = 32;

/// The number of bits that identify two chunks in the same region
pub const SUBREGION_BITS: u8 = pumpkin_util::math::ceil_log2(REGION_SIZE as u32);

pub const SUBREGION_AND: i32 = i32::pow(2, SUBREGION_BITS as u32) - 1;

/// The number of chunks in a region
pub const CHUNK_COUNT: usize = REGION_SIZE * REGION_SIZE;

/// The number of bytes in a sector (4 KiB)
const SECTOR_BYTES: usize = 4096;

/// The maximum sector count representable in the one-byte count field of a
/// location entry. Chunks that would need more sectors are stored as external
/// chunks (`.mcc`), exactly like vanilla, so the field can never overflow.
const MAX_IN_FILE_SECTORS: u32 = 255;

/// Flag OR'ed into the compression version byte of chunks whose payload lives
/// in an external `c.<x>.<z>.mcc` file (vanilla external chunk mechanism).
const EXTERNAL_FLAG: u8 = 0x80;

/// Keys of the oversized compounds merged back on load (Paper
/// `Allow Saving of Oversized Chunks` compatibility).
const OVERSIZED_MERGE_KEYS: [&str; 3] = ["Entities", "block_entities", "TileEntities"];

// 1.21.11
pub const WORLD_DATA_VERSION: i32 = 4671;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Compression {
    /// `GZip` Compression
    GZip = Self::GZIP_ID,
    /// `ZLib` Compression
    ZLib = Self::ZLIB_ID,
    /// LZ4 Compression (since 24w04a)
    LZ4 = Self::LZ4_ID,
    /// Custom compression algorithm (since 24w05a)
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
    // Length is always the length of this + compression byte (1) so we dont need to save a length
    compressed_data: Bytes,
    /// The payload lives in an external `c.<x>.<z>.mcc` file; the region file
    /// only holds the five byte header. When such a chunk was read from disk,
    /// the payload is filled in by [`AnvilChunkFile::read_at`].
    external: bool,
}

enum WriteAction {
    // Don't write anything
    Pass,
    // Write the entire file
    All,
    // Only write certain indices
    Parts(Vec<usize>),
}

impl WriteAction {
    /// If we are currently not writing, sets to new Parts enum,
    /// If we have parts enum, add to it,
    /// If we have All enum, do nothing
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

    // NOTE: This is only valid if our WriteAction is `Parts`
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

    fn decompress_data(self, compressed_data: &[u8]) -> Result<Box<[u8]>, CompressionError> {
        fn decode<R: std::io::Read>(mut reader: R, capacity: usize) -> std::io::Result<Box<[u8]>> {
            let mut buf = Vec::with_capacity(capacity);
            reader.read_to_end(&mut buf)?;
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

    /// Returns Ok when a compression is found otherwise an Err
    #[expect(clippy::result_unit_err)]
    pub const fn from_byte(byte: u8) -> Result<Option<Self>, ()> {
        match byte {
            Self::GZIP_ID => Ok(Some(Self::GZip)),
            Self::ZLIB_ID => Ok(Some(Self::ZLib)),
            // Uncompressed (since a version before 1.15.1)
            Self::NO_COMPRESSION_ID => Ok(None),
            Self::LZ4_ID => Ok(Some(Self::LZ4)),
            Self::CUSTOM_ID => Ok(Some(Self::Custom)),
            // Unknown format
            _ => Err(()),
        }
    }
}

impl From<pumpkin_config::chunk::Compression> for Compression {
    fn from(value: pumpkin_config::chunk::Compression) -> Self {
        // :c
        match value {
            pumpkin_config::chunk::Compression::GZip => Self::GZip,
            pumpkin_config::chunk::Compression::ZLib => Self::ZLib,
            pumpkin_config::chunk::Compression::LZ4 => Self::LZ4,
            pumpkin_config::chunk::Compression::Custom => Self::Custom,
        }
    }
}

impl AnvilChunkData {
    /// Raw size of serialized chunk
    #[inline]
    const fn raw_write_size(&self) -> usize {
        if self.external {
            // Only the five byte header goes into the region file
            return 4 + 1;
        }
        // 4 bytes for the *length* and 1 byte for the *compression* method
        self.compressed_data.len() + 4 + 1
    }

    /// Size of serialized chunk with padding
    #[inline]
    const fn padded_size(&self) -> usize {
        let sector_count = self.sector_count() as usize;
        sector_count * SECTOR_BYTES
    }

    /// Number of sectors this chunk occupies inside the region file. External
    /// chunks only occupy a single sector holding their five byte header.
    #[inline]
    const fn sector_count(&self) -> u32 {
        if self.external {
            return 1;
        }
        let total_size = self.raw_write_size();
        total_size.div_ceil(SECTOR_BYTES) as u32
    }

    /// Whether the payload is too large to be stored inside the region file.
    #[inline]
    const fn is_oversized(&self) -> bool {
        !self.external && self.sector_count() > MAX_IN_FILE_SECTORS
    }

    fn from_bytes(bytes: Bytes) -> Result<Self, ChunkReadingError> {
        let mut bytes = bytes;
        // Minus one for the compression byte, which the length covers, so
        // anything below one does not describe a chunk at all.
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
            // The payload lives in an external file; the region file only
            // stores the five byte header. Nothing to slice here.
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
            // If this has padding, we need to trim it
            compressed_data: bytes.slice(..length),
            external: false,
        })
    }

    async fn write(&self, w: &mut (impl AsyncWrite + Unpin + Send)) -> Result<(), std::io::Error> {
        let padded_size = self.padded_size();

        // The declared length always covers the payload plus the compression
        // byte, even when the payload itself lives in an external file.
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

/// Parses the region coordinates out of a `r.<x>.<z>.mca` file name.
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

/// Path of the vanilla external chunk file for an absolute chunk position.
fn external_chunk_path(region_path: &Path, abs_x: i32, abs_z: i32) -> PathBuf {
    region_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("c.{abs_x}.{abs_z}.mcc"))
}

/// Path of the Paper oversized-chunk sidecar file for an absolute chunk
/// position (`Allow Saving of Oversized Chunks`).
fn paper_oversized_chunk_path(region_path: &Path, abs_x: i32, abs_z: i32) -> Option<PathBuf> {
    let name = region_path.file_name()?.to_str()?.strip_suffix(".mca")?;
    Some(
        region_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("{name}_oversized_{abs_x}_{abs_z}.nbt")),
    )
}

/// Path of the Paper oversized-chunk bitmap sidecar for a whole region.
fn paper_oversized_meta_path(region_path: &Path) -> Option<PathBuf> {
    let name = region_path.file_name()?.to_str()?.strip_suffix(".mca")?;
    Some(
        region_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("{name}.oversized.nbt")),
    )
}

/// Reads the Paper oversized bitmap, returning `[bool; CHUNK_COUNT]`.
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
            warn!(
                "Failed to read oversized meta file {}: {err}",
                meta_path.display()
            );
        }
    }
    flags
}

/// Merges the oversized sidecar compound into the base chunk NBT.
fn merge_paper_oversized(
    base_nbt: &mut pumpkin_nbt::NbtCompound,
    oversized: &pumpkin_nbt::NbtCompound,
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
                if matches!(extra.first(), Some(pumpkin_nbt::tag::NbtTag::Compound(_))) =>
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

/// Atomically writes `bytes` to `path` through a temp file. `sync_all` is
/// issued before the rename so the data is on disk once this returns.
async fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let temp_path = path.with_extension("tmp_atomic");
    let mut file = tokio::fs::File::create(&temp_path).await?;
    file.write_all(bytes).await?;
    file.flush().await?;
    file.sync_all().await?;
    drop(file);
    tokio::fs::rename(&temp_path, path).await
}

impl<S: SingleChunkDataSerializer> AnvilChunkFile<S> {
    #[must_use]
    pub const fn get_region_coords(at: &Vector2<i32>) -> (i32, i32) {
        // Divide by 32 for the region coordinates
        (at.x >> SUBREGION_BITS, at.y >> SUBREGION_BITS)
    }

    #[must_use]
    pub const fn get_chunk_index(x: i32, z: i32) -> usize {
        let local_x = x & SUBREGION_AND;
        let local_z = z & SUBREGION_AND;
        let index = (local_z << SUBREGION_BITS) + local_x;
        index as usize
    }

    /// Absolute chunk coordinates for a chunk index inside the region the given
    /// path points to.
    fn absolute_chunk_pos(region_path: &Path, index: usize) -> Option<(i32, i32)> {
        let (region_x, region_z) = region_coords_from_path(region_path)?;
        let local_x = (index % REGION_SIZE) as i32;
        let local_z = (index / REGION_SIZE) as i32;
        Some((
            region_x * REGION_SIZE as i32 + local_x,
            region_z * REGION_SIZE as i32 + local_z,
        ))
    }

    /// Reads an external `.mcc` payload for a chunk, if present.
    fn read_external_payload(region_path: &Path, index: usize) -> Option<Bytes> {
        let (abs_x, abs_z) = Self::absolute_chunk_pos(region_path, index)?;
        match std::fs::read(external_chunk_path(region_path, abs_x, abs_z)) {
            Ok(data) => Some(data.into()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => {
                warn!("Failed to read external chunk file for chunk {abs_x}/{abs_z}: {err}");
                None
            }
        }
    }

    /// Fills the payloads of chunks read as external from their `.mcc` files.
    /// Missing files leave the payload empty, which surfaces as a per-chunk
    /// error instead of failing the whole region.
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
                        "External chunk file for index {index} of {} is missing; the chunk \
                         cannot be loaded",
                        region_path.display()
                    );
                }
            }
        }
    }

    /// Applies the Paper oversized sidecar data to a freshly parsed region, if
    /// any. Chunks flagged oversized get their entity data merged back into the
    /// base payload so the rest of the pipeline sees one normal chunk.
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
                // The sidecar is a zlib compressed named-root NBT document; the
                // streaming decoder cannot seek, so decode it in memory first.
                let mut decoded = Vec::new();
                ZlibDecoder::new(&compressed[..])
                    .read_to_end(&mut decoded)
                    .map_err(std::io::Error::other)?;
                let mut cursor = std::io::Cursor::new(decoded);
                let mut reader = pumpkin_nbt::deserializer::NbtReadHelperJava::new(
                    pumpkin_nbt::deserializer::NbtStreamReader(&mut cursor),
                );
                pumpkin_nbt::Nbt::read(&mut reader)
                    .map_err(|err| {
                        std::io::Error::other(format!("failed to parse oversized NBT: {err}"))
                    })
                    .map(|nbt| nbt.root_tag)
            });
            let oversized_nbt = match oversized_nbt {
                Ok(nbt) => nbt,
                Err(err) => {
                    warn!(
                        "Failed to read oversized chunk data {} for chunk {abs_x}/{abs_z}: {err}",
                        oversized_path.display()
                    );
                    continue;
                }
            };

            // Decompress the base payload, merge the oversized compound and
            // store the merged raw payload back.
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
            let mut reader = pumpkin_nbt::deserializer::NbtReadHelperJava::new(
                pumpkin_nbt::deserializer::NbtStreamReader(&mut cursor),
            );
            let Ok(pumpkin_nbt::Nbt {
                root_tag: mut base_nbt,
                ..
            }) = pumpkin_nbt::Nbt::read_unnamed(&mut reader)
            else {
                continue;
            };

            merge_paper_oversized(&mut base_nbt, &oversized_nbt);

            metadata.serialized_data = AnvilChunkData {
                compression: None,
                compressed_data: pumpkin_nbt::Nbt::from(base_nbt).write_unnamed(),
                external: false,
            };
        }
    }

    /// Writes the external payloads of all external chunks and cleans up stale
    /// external files of chunks that are stored inline again.
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
                // The chunk is stored inline; a leftover external file from an
                // earlier save would be ignored by readers, but remove it to
                // avoid confusion and wasted disk space.
                match tokio::fs::remove_file(&external_path).await {
                    Ok(()) => {}
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                    Err(err) => {
                        warn!(
                            "Failed to remove stale external chunk file {}: {err}",
                            external_path.display()
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// Removes the Paper oversized sidecars for chunks now stored inline.
    /// Called after a successful write so the oversized data can never shadow
    /// the freshly saved chunk when read back by Paper/Papo.
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
                warn!(
                    "Failed to remove oversized chunk file {}: {err}",
                    path.display()
                );
            }
        }

        if let Err(err) = tokio::fs::remove_file(&meta_path).await
            && err.kind() != std::io::ErrorKind::NotFound
        {
            warn!(
                "Failed to remove oversized meta file {}: {err}",
                meta_path.display()
            );
        }
    }

    /// Assigns every present chunk a fresh sequential sector offset, matching
    /// the exact layout [`Self::write_all`] produces. Must be called whenever
    /// the write action escalates to `All`, so that the in-memory offsets never
    /// describe a file layout that no longer exists.
    ///
    /// Takes the two fields directly (instead of `&mut self`) so callers can
    /// hold the `write_action` lock while renumbering.
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
        trace!("Writing in place: {}", path.display());

        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .append(false)
            .open(path)
            .await?;

        let mut write = BufWriter::new(file);
        // The first two sectors are reserved for the location table
        let mut header = Vec::with_capacity(SECTOR_BYTES * 2);

        // Location Table
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

        // Timestamp Table
        for metadata in &self.chunks_data {
            if let Some(chunk) = metadata {
                header.put_u32(chunk.timestamp);
            } else {
                header.put_u32(0);
            }
        }

        // Write all 8 KiB in a single async call
        write.write_all(&header).await?;

        let mut chunks = indices
            .into_iter()
            .filter_map(|index| self.chunks_data[index].as_ref().map(|c| (index, c)))
            .collect::<Vec<_>>();

        // Sort such that writes are in order
        chunks.sort_by_key(|chunk| chunk.1.file_sector_offset);

        #[cfg(debug_assertions)]
        {
            // Verify we are actually two sectors into the file
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

            // Seek only if we need to
            if chunk.file_sector_offset != current_sector {
                trace!("Seeking to sector {}", chunk.file_sector_offset);
                let _ = write
                    .seek(SeekFrom::Start(
                        chunk.file_sector_offset as u64 * SECTOR_BYTES as u64,
                    ))
                    .await?;
                current_sector = chunk.file_sector_offset;
            }
            trace!(
                "Writing chunk {} - {}:{}",
                index,
                current_sector,
                chunk.serialized_data.sector_count()
            );

            current_sector += chunk.serialized_data.sector_count();

            chunk.serialized_data.write(&mut write).await?;
        }

        write.flush().await?;
        write.get_ref().sync_all().await?;

        // External payloads need to be persisted too; chunks that became
        // inline get their stale external files removed.
        self.write_external_payloads(path).await?;

        Ok(())
    }

    /// Write entire file, disregarding saved offsets. This is the safe default:
    /// the file is built from scratch next to the original and atomically
    /// swapped in, so a crash mid-write can never produce a torn region file.
    async fn write_all(&self, path: &Path) -> Result<(), std::io::Error> {
        let temp_path = path.with_extension("tmp");
        trace!("Writing tmp file to disk: {temp_path:?}");

        let file = tokio::fs::File::create(&temp_path).await?;
        let mut write = BufWriter::new(file);

        // Build the 8 KiB header in memory
        let mut header = Vec::with_capacity(SECTOR_BYTES * 2);
        let mut current_sector: u32 = 2;

        // Location Table
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

        // Timestamp Table
        for metadata in &self.chunks_data {
            if let Some(chunk) = metadata {
                header.put_u32(chunk.timestamp);
            } else {
                header.put_u32(0);
            }
        }

        // Write all 8 KiB in a single async call
        write.write_all(&header).await?;

        // Write chunk data
        for chunk in self.chunks_data.iter().flatten() {
            chunk.serialized_data.write(&mut write).await?;
        }

        write.flush().await?;
        write.get_ref().sync_all().await?;

        // The assigned layout must match what `renumber_sectors` recorded; if
        // it does not, an in-place follow-up write would corrupt the file.
        // The whole statement is cfg-gated: `debug_assert_eq!` still
        // name-resolves its arguments when debug assertions are off.
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

        // External payloads are written (atomically) before the region file is
        // swapped in, so a successful rename implies a complete save.
        self.write_external_payloads(path).await?;

        tokio::fs::rename(temp_path, path).await?;

        // With every chunk stored inside the region file again, the Paper
        // oversized sidecars must go or they would shadow the new data.
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
            // Two sectors for offset + timestamp
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
        // Locked means a write is in flight right now; be conservative.
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
                debug!(
                    "Skipping write for {}, as there were no dirty chunks",
                    path.display()
                );
                Ok(())
            }
            WriteAction::All => self.write_all(path).await,
            WriteAction::Parts(parts) => self.write_indices(path, parts.iter().copied()).await,
        }?;

        // If we still are in memory after this, we don't need to write again!
        // On failure the `?` above returns early and the action is kept, so the
        // next write retries instead of silently dropping the data.
        *write_action = WriteAction::Pass;
        Ok(())
    }

    /// Parses a region file. External chunk payloads are read from their
    /// `c.<x>.<z>.mcc` sidecars and Paper oversized sidecars are merged back
    /// in, based on `path`.
    fn read_at(r: Bytes, path: &Path) -> Result<Self, ChunkReadingError> {
        let mut file = Self::read(r)?;
        file.fill_external_payloads(path);
        file.apply_paper_oversized(path);
        Ok(file)
    }

    fn read(r: Bytes) -> Result<Self, ChunkReadingError> {
        let mut raw_file_bytes = r;

        // A file smaller than the 8 KiB header cannot contain chunk data; the
        // header must have been torn by a crash. Treat it as empty instead of
        // failing the whole region.
        if raw_file_bytes.is_empty() {
            return Ok(Self::default());
        }

        if raw_file_bytes.len() < SECTOR_BYTES * 2 {
            warn!(
                "Region file of {} bytes is smaller than its 8 KiB header; treating as empty",
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

            // If the sector offset or count is 0, the chunk is not present (we should not parse empty chunks).
            // Sector 1 is the timestamp table, so a chunk cannot start there either.
            if sector_offset < 2 || sector_count == 0 {
                continue;
            }

            // Spigot extension: the chunk needed more sectors than the one-byte
            // count field can express; the real sector count is derived from
            // the length prefix stored at the chunk's offset.
            if sector_count == MAX_IN_FILE_SECTORS as usize {
                let length_offset = (sector_offset - 2) * SECTOR_BYTES;
                let Some(declared_length) = raw_file_bytes
                    .get(length_offset..length_offset + 4)
                    .and_then(|length| <[u8; 4]>::try_from(length).ok())
                    .map(u32::from_be_bytes)
                else {
                    warn!("Region entry {i} marked as 255 sectors but out of bounds; dropping it");
                    continue;
                };
                sector_count = (declared_length as usize + 4) / SECTOR_BYTES + 1;
            }

            let end_offset = sector_offset + sector_count;

            // We always subtract 2 for the first two sectors for the timestamp and location tables
            // that we walked earlier
            let bytes_offset = (sector_offset - 2) * SECTOR_BYTES;
            let bytes_count = sector_count * SECTOR_BYTES;

            if bytes_offset + bytes_count > raw_file_bytes.len() {
                // Header self-heal: a corrupt entry must not take the whole
                // region file down; drop the entry and keep the others.
                warn!(
                    "Region entry {i} (sectors {sector_offset}..{end_offset}) is out of bounds \
                     ({} bytes); dropping it",
                    raw_file_bytes.len()
                );
                continue;
            }

            let serialized_data = match AnvilChunkData::from_bytes(
                raw_file_bytes.slice(bytes_offset..bytes_offset + bytes_count),
            ) {
                Ok(data) => data,
                Err(err) => {
                    warn!("Region entry {i} failed to parse ({err:?}); dropping it");
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
        // Default to the compression type read from the file
        let compression_type = self.chunks_data[index]
            .as_ref()
            .and_then(|chunk_data| chunk_data.serialized_data.compression);
        let chunk_config_snapshot = chunk_config.clone();
        let mut new_chunk_data = run_blocking(move || {
            AnvilChunkData::from_chunk(&*chunk, compression_type, &chunk_config_snapshot)
        })
        .await
        .map_err(|_| {
            ChunkWritingError::IoError(std::io::Error::other("chunk serialization task failed"))
        })??;

        // Chunks whose payload exceeds the 255 sector limit of the location
        // table go to an external `c.<x>.<z>.mcc` file, exactly like vanilla.
        // The one-byte sector count can never overflow this way.
        if new_chunk_data.is_oversized() {
            new_chunk_data.external = true;
        }

        let mut write_action = self.write_action.lock().await;
        if !chunk_config.write_in_place {
            *write_action = WriteAction::All;
        }

        match &*write_action {
            WriteAction::All => {
                trace!("Write action is all: setting chunk in place");
                self.chunks_data[index] = Some(AnvilChunkMetadata {
                    serialized_data: new_chunk_data,
                    timestamp: epoch,
                    file_sector_offset: 0,
                });
                // Keep the in-memory offsets in sync with the layout the full
                // rewrite produces, so later in-place writes stay valid.
                Self::renumber_sectors(&mut self.chunks_data, &mut self.end_sector);
            }
            _ => {
                match self.chunks_data[index].as_ref() {
                    None => {
                        trace!(
                            "Chunk {} does not exist, appending to EOF: {}:{}",
                            index,
                            self.end_sector,
                            new_chunk_data.sector_count()
                        );
                        // This chunk didn't exist before; append to EOF
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
                                "Chunk {} exists, writing in place: {}:{}",
                                index,
                                old_chunk.file_sector_offset,
                                new_chunk_data.sector_count()
                            );
                            // We can just add it (this also covers external
                            // chunks: same-size means only the `.mcc` payload
                            // and the header byte change)
                            self.chunks_data[index] = Some(AnvilChunkMetadata {
                                serialized_data: new_chunk_data,
                                timestamp: epoch,
                                file_sector_offset: old_chunk.file_sector_offset,
                            });
                            write_action.maybe_update_chunk_index(index);
                        } else {
                            // The chunk changed size. Re-shuffling sectors in
                            // place leaves a window where a crash corrupts
                            // neighbouring chunks, so fall back to the atomic
                            // full rewrite instead.
                            trace!(
                                "Chunk {} changed size, falling back to atomic full rewrite",
                                index
                            );
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
    use pumpkin_config::chunk::ChunkCompression;
    use pumpkin_nbt::tag::NbtTag;

    fn config_with(write_in_place: bool) -> AnvilChunkConfig {
        AnvilChunkConfig {
            compression: ChunkCompression::default(),
            write_in_place,
        }
    }

    /// A tiny chunk payload carrier for serializer-level tests. It also records
    /// which non-payload keys survived a round-trip, to observe merges.
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
            let mut root = pumpkin_nbt::NbtCompound::new();
            root.put_int("x", self.x);
            root.put_int("z", self.z);
            // Split the payload: `pumpkin-nbt` refuses to read arrays larger
            // than `MAX_ARRAY_LENGTH` (512_000) elements.
            for (part_index, part) in self.payload.chunks(500_000).enumerate() {
                root.put(
                    &format!("payload_{part_index}"),
                    NbtTag::ByteArray(part.iter().map(|&b| b as i8).collect()),
                );
            }
            Ok(pumpkin_nbt::Nbt::from(root).write_unnamed())
        }

        fn from_bytes(bytes: &Bytes, pos: Vector2<i32>) -> Result<Self, ChunkReadingError> {
            let mut cursor = std::io::Cursor::new(bytes.as_ref());
            let mut reader = pumpkin_nbt::deserializer::NbtReadHelperJava::new(
                pumpkin_nbt::deserializer::NbtStreamReader(&mut cursor),
            );
            let nbt = pumpkin_nbt::Nbt::read_unnamed(&mut reader).map_err(|e| {
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
        encoder.read_to_end(&mut out).expect("zlib encode failed");
        out
    }

    fn mock_nbt_bytes(payload: &[u8]) -> Vec<u8> {
        let mut root = pumpkin_nbt::NbtCompound::new();
        root.put_int("x", 0);
        root.put_int("z", 0);
        root.put(
            "payload_0",
            NbtTag::ByteArray(payload.iter().map(|&b| b as i8).collect()),
        );
        pumpkin_nbt::Nbt::from(root).write_unnamed().to_vec()
    }

    /// Builds a raw region file: header + one chunk per spec.
    /// `entries`: (index, timestamp, sector-aligned record incl. 5-byte header)
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
            .expect("runtime")
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
        // The chunk needs more sectors than the one-byte count field can
        // express; the count byte is saturated at 255 and readers must
        // recompute the real count from the length prefix.
        let payload = zlib_wrap(&mock_nbt_bytes(&vec![0xAB; 300 * 1024]));
        let record = chunk_record(Some(Compression::ZLib as u8), &payload);
        let mut file = build_region(&[(0, 42, record)]);
        // Saturate the count byte for entry 0 (offset stays sector 2).
        file[0..4].copy_from_slice(&((2u32 << 8) | 255).to_be_bytes());

        let region =
            <AnvilChunkFile<MockChunk> as ChunkSerializer>::read(file.into()).expect("parse ok");
        let results = load_all(&region, vec![Vector2::new(0, 0)]);
        match &results[0] {
            LoadedData::Loaded(chunk) => {
                assert_eq!(chunk.payload.len(), 300 * 1024);
                assert!(chunk.payload.iter().all(|&b| b == 0xAB));
            }
            other => panic!("expected loaded chunk, got {other:?}"),
        }
    }

    #[test]
    fn read_heals_out_of_bounds_entries() {
        // Entry claims sectors far beyond the file: it must be dropped without
        // failing the whole region, and the second entry must still be there.
        let good_payload = zlib_wrap(&mock_nbt_bytes(b"good"));
        let good = chunk_record(Some(Compression::ZLib as u8), &good_payload);
        let mut file = build_region(&[(5, 1, good)]);
        // Entry 0 points outside the file.
        file[0..4].copy_from_slice(&((9000u32 << 8) | 3).to_be_bytes());

        let region =
            <AnvilChunkFile<MockChunk> as ChunkSerializer>::read(file.into()).expect("parse ok");
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
            <AnvilChunkFile<MockChunk> as ChunkSerializer>::read(file.into()).expect("parse ok");
        let results = load_all(&region, vec![Vector2::new(0, 0)]);
        assert!(matches!(results[0], LoadedData::Missing(_)));
    }

    #[test]
    fn external_entry_with_missing_mcc_is_per_chunk_error() {
        let temp = tempfile::tempdir().expect("tempdir");
        let region_path = temp.path().join("r.0.0.mca");

        // Entry 0 with external flag set but no `.mcc` file next to it.
        let header = chunk_record(Some(Compression::ZLib as u8 | 0x80), b"");
        let file = build_region(&[(0, 7, header)]);

        let region =
            <AnvilChunkFile<MockChunk> as ChunkSerializer>::read_at(file.into(), &region_path)
                .expect("parse ok");
        let results = load_all(&region, vec![Vector2::new(0, 0)]);
        assert!(matches!(results[0], LoadedData::Error(_)));
    }

    #[test]
    fn paper_oversized_data_is_merged_on_read() {
        let temp = tempfile::tempdir().expect("tempdir");
        let region_path = temp.path().join("r.0.0.mca");

        // Base chunk 0/0 without entities, stored zlib inline.
        let base_payload = zlib_wrap(&mock_nbt_bytes(b"base"));
        let record = chunk_record(Some(Compression::ZLib as u8), &base_payload);
        let file = build_region(&[(0, 1, record)]);
        std::fs::write(&region_path, &file).expect("write region");

        // Oversized sidecar holding the extracted entity data.
        let mut root = pumpkin_nbt::NbtCompound::new();
        let mut entity = pumpkin_nbt::NbtCompound::new();
        entity.put_string("id", "minecraft:zombie".to_string());
        root.put_list("Entities", vec![NbtTag::Compound(entity)]);
        let sidecar = pumpkin_nbt::Nbt::new(String::new(), root);
        let sidecar_path = temp.path().join("r.0.0_oversized_0_0.nbt");
        std::fs::write(&sidecar_path, zlib_wrap(&sidecar.write())).expect("write sidecar");

        // Bitmap marking chunk 0 as oversized.
        let mut meta = vec![0u8; CHUNK_COUNT];
        meta[0] = 1;
        std::fs::write(temp.path().join("r.0.0.oversized.nbt"), meta).expect("write meta");

        let region =
            <AnvilChunkFile<MockChunk> as ChunkSerializer>::read_at(file.into(), &region_path)
                .expect("parse ok");
        let results = load_all(&region, vec![Vector2::new(0, 0)]);
        match &results[0] {
            LoadedData::Loaded(chunk) => {
                assert_eq!(chunk.payload, b"base");
                assert!(chunk.extra_keys.iter().any(|key| key == "Entities"));
            }
            other => panic!("expected loaded chunk, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn oversized_chunk_written_as_external_never_overflows_location_table() {
        let temp = tempfile::tempdir().expect("tempdir");
        let region_path = temp.path().join("r.0.0.mca");

        // Payload large enough to need more than 255 sectors (~1 MiB) once
        // compressed: incompressible xorshift bytes, not a repeated pattern.
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
            .expect("update ok");

        region.write(&region_path).await.expect("write ok");

        // The location entry must stay representable: count == 1 sector, and
        // the header byte at that offset must carry the external flag.
        let bytes = tokio::fs::read(&region_path).await.expect("read back");
        let entry_index = AnvilChunkFile::<MockChunk>::get_chunk_index(3, 4);
        let entry_slice = &bytes[entry_index * 4..entry_index * 4 + 4];
        let entry = u32::from_be_bytes(entry_slice.try_into().expect("entry 4 bytes"));
        let (offset, count) = (entry >> 8, entry & 0xFF);
        assert_eq!(count, 1, "external chunks occupy a single header sector");
        let header_byte = bytes[offset as usize * SECTOR_BYTES + 4];
        assert_ne!(
            header_byte & EXTERNAL_FLAG,
            0,
            "header byte must carry the external flag"
        );
        assert!(Compression::from_byte(header_byte & !EXTERNAL_FLAG).is_ok());

        // The external payload file must exist.
        assert!(temp.path().join("c.3.4.mcc").exists());

        // Round trip: read_at must load the chunk from the external file.
        let region =
            <AnvilChunkFile<MockChunk> as ChunkSerializer>::read_at(bytes.into(), &region_path)
                .expect("parse");
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        region.get_chunks(vec![Vector2::new(3, 4)], tx).await;
        while let Some(item) = rx.recv().await {
            match item {
                LoadedData::Loaded(chunk) => assert_eq!(chunk.payload, payload),
                other => panic!("expected loaded chunk, got {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn atomic_rewrite_round_trip() {
        let temp = tempfile::tempdir().expect("tempdir");
        let region_path = temp.path().join("r.0.0.mca");

        let mut region: AnvilChunkFile<MockChunk> = AnvilChunkFile::default();
        region
            .update_chunk(MockChunk::new(0, 0, b"hello".to_vec()), &config_with(false))
            .await
            .expect("update 1");
        region
            .update_chunk(
                MockChunk::new(31, 31, b"world".to_vec()),
                &config_with(false),
            )
            .await
            .expect("update 2");
        region.write(&region_path).await.expect("write");

        let raw = tokio::fs::read(&region_path).await.expect("read");
        let region =
            <AnvilChunkFile<MockChunk> as ChunkSerializer>::read_at(raw.into(), &region_path)
                .expect("parse");
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

        // No leftover temp files.
        let mut entries = tokio::fs::read_dir(temp.path()).await.expect("readdir");
        while let Some(entry) = entries.next_entry().await.expect("entry") {
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
        // Regression for the stale-offset hazard: after an atomic full rewrite
        // (All), a follow-up same-size in-place write must still land on the
        // correct sector, not on the old file layout.
        let temp = tempfile::tempdir().expect("tempdir");
        let region_path = temp.path().join("r.0.0.mca");

        let mut region: AnvilChunkFile<MockChunk> = AnvilChunkFile::default();
        // First save: default (atomic All) path.
        region
            .update_chunk(MockChunk::new(2, 2, b"first".to_vec()), &config_with(false))
            .await
            .expect("update 1");
        region.write(&region_path).await.expect("write 1");

        // Second save: in-place mode, same size payload.
        region
            .update_chunk(MockChunk::new(2, 2, b"secnd".to_vec()), &config_with(true))
            .await
            .expect("update 2");
        region.write(&region_path).await.expect("write 2");

        let raw = tokio::fs::read(&region_path).await.expect("read");
        let region =
            <AnvilChunkFile<MockChunk> as ChunkSerializer>::read_at(raw.into(), &region_path)
                .expect("parse");
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        region.get_chunks(vec![Vector2::new(2, 2)], tx).await;
        while let Some(item) = rx.recv().await {
            match item {
                LoadedData::Loaded(chunk) => assert_eq!(chunk.payload, b"secnd"),
                other => panic!("expected loaded chunk, got {other:?}"),
            }
        }
    }

    #[test]
    fn compression_round_trip_all_variants() {
        for compression in [Compression::GZip, Compression::ZLib, Compression::LZ4] {
            let data = b"the quick brown fox jumps over the lazy dog".repeat(64);
            let compressed = compression
                .compress_data(&data, 6)
                .expect("compress failed");
            let decompressed = compression
                .decompress_data(&compressed)
                .expect("decompress failed");
            assert_eq!(decompressed.as_ref(), data.as_slice(), "{compression:?}");
        }
    }
}
