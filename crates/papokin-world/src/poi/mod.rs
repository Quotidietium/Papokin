use rustc_hash::FxHashMap;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;
use serde::{Deserialize, Serialize};

/// 下界传送门的 POI 类型标识符
pub const POI_TYPE_NETHER_PORTAL: &str = "minecraft:nether_portal";

/// MCA 格式常量
const SECTOR_SIZE: usize = 4096;
const REGION_SIZE: usize = 32;
const CHUNK_COUNT: usize = REGION_SIZE * REGION_SIZE;
const HEADER_SIZE: usize = SECTOR_SIZE * 2; // 位置表 + 时间戳表

/// MCA 格式的压缩类型
const COMPRESSION_ZLIB: u8 = 2;

// 1.21.11 的数据版本
const DATA_VERSION: i32 = 4671;

/// 单个兴趣点（Point of Interest）条目（可序列化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoiEntry {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    #[serde(rename = "type")]
    pub poi_type: String,
    pub free_tickets: i32,
}

impl PoiEntry {
    #[must_use]
    pub fn new_portal(pos: BlockPos) -> Self {
        Self {
            x: pos.0.x,
            y: pos.0.y,
            z: pos.0.z,
            poi_type: POI_TYPE_NETHER_PORTAL.to_string(),
            free_tickets: 0,
        }
    }

    #[must_use]
    pub const fn pos(&self) -> BlockPos {
        BlockPos(Vector3::new(self.x, self.y, self.z))
    }
}

/// POI 区段数据（可序列化）- 原版格式
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PoiSectionData {
    #[serde(default)]
    pub valid: i8,
    #[serde(default)]
    pub records: Vec<PoiEntry>,
}

/// POI 区块数据（可序列化）- 原版格式
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PoiChunkData {
    pub data_version: i32,
    /// 以 Y 区段坐标为键的区段（例如 "-1"、"0"、"1"、"4"）
    pub sections: FxHashMap<String, PoiSectionData>,
}

/// 以 MCA 格式存储单个区域（32x32 区块）的 POI 数据
#[derive(Debug, Default)]
pub struct PoiRegion {
    /// 按位置索引的条目
    entries: FxHashMap<(i32, i32, i32), PoiEntry>,
    /// 跟踪哪些区块是脏区块
    dirty_chunks: rustc_hash::FxHashSet<(i32, i32)>,
    dirty: bool,
}

impl PoiRegion {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    const fn pos_key(pos: &BlockPos) -> (i32, i32, i32) {
        (pos.0.x, pos.0.y, pos.0.z)
    }

    /// 获取 MCA 文件中的区块索引（0-1023）
    const fn chunk_index(chunk_x: i32, chunk_z: i32) -> usize {
        let local_x = chunk_x & 31;
        let local_z = chunk_z & 31;
        ((local_z << 5) | local_x) as usize
    }

    ///返回区块段键，仅使用 Y 区块段坐标（与原版一致）
    fn section_key(pos: &BlockPos) -> String {
        let section_y = pos.0.y >> 4;
        section_y.to_string()
    }

    pub fn add(&mut self, entry: PoiEntry) {
        let chunk_x = entry.x >> 4;
        let chunk_z = entry.z >> 4;
        self.dirty_chunks.insert((chunk_x, chunk_z));
        let key = (entry.x, entry.y, entry.z);
        self.entries.insert(key, entry);
        self.dirty = true;
    }

    pub fn remove(&mut self, pos: &BlockPos) -> bool {
        let key = Self::pos_key(pos);
        if self.entries.remove(&key).is_some() {
            let chunk_x = pos.0.x >> 4;
            let chunk_z = pos.0.z >> 4;
            self.dirty_chunks.insert((chunk_x, chunk_z));
            self.dirty = true;
            return true;
        }
        false
    }

    #[must_use]
    pub fn get_all(&self) -> Vec<&PoiEntry> {
        self.entries.values().collect()
    }

    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
        self.dirty_chunks.clear();
    }

    /// 按区块对条目分组，然后创建区块 NBT 数据
    fn get_chunk_data(&self, chunk_x: i32, chunk_z: i32) -> Option<PoiChunkData> {
        let mut sections: FxHashMap<String, PoiSectionData> = FxHashMap::default();

        for entry in self.entries.values() {
            let entry_chunk_x = entry.x >> 4;
            let entry_chunk_z = entry.z >> 4;

            if entry_chunk_x != chunk_x || entry_chunk_z != chunk_z {
                continue;
            }

            let section_key = Self::section_key(&entry.pos());
            let section = sections
                .entry(section_key)
                .or_insert_with(|| PoiSectionData {
                    valid: 1,
                    records: Vec::new(),
                });
            section.records.push(entry.clone());
        }

        if sections.is_empty() {
            None
        } else {
            Some(PoiChunkData {
                data_version: DATA_VERSION,
                sections,
            })
        }
    }

    /// 将区块数据压缩为字节
    fn compress_chunk_data(chunk_data: &PoiChunkData) -> std::io::Result<Vec<u8>> {
        let mut root = papokin_nbt::compound::NbtCompound::new();
        root.put_int("DataVersion", chunk_data.data_version);

        let mut sections_comp = papokin_nbt::compound::NbtCompound::new();
        for (sec_key, sec_data) in &chunk_data.sections {
            let mut sec_comp = papokin_nbt::compound::NbtCompound::new();
            sec_comp.put_byte("Valid", sec_data.valid);
            let mut rec_list = Vec::new();
            for rec in &sec_data.records {
                let mut rec_comp = papokin_nbt::compound::NbtCompound::new();
                rec_comp.put_int("x", rec.x);
                rec_comp.put_int("y", rec.y);
                rec_comp.put_int("z", rec.z);
                rec_comp.put_string("type", rec.poi_type.clone());
                rec_comp.put_int("free_tickets", rec.free_tickets);
                rec_list.push(papokin_nbt::tag::NbtTag::Compound(rec_comp));
            }
            sec_comp.put_list("Records", rec_list);
            sections_comp.put_compound(sec_key, sec_comp);
        }
        root.put_compound("Sections", sections_comp);

        let uncompressed = papokin_nbt::Nbt::from(root).write();
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&uncompressed)?;
        encoder.finish()
    }

    /// 从字节数据解压区块数据
    fn decompress_chunk_data(compressed: &[u8]) -> std::io::Result<PoiChunkData> {
        let mut decoder = ZlibDecoder::new(compressed);
        let mut uncompressed = Vec::new();
        decoder.read_to_end(&mut uncompressed)?;

        let mut cursor = Cursor::new(uncompressed);
        let mut reader = papokin_nbt::deserializer::NbtReadHelperJava::new(
            papokin_nbt::deserializer::NbtStreamReader(&mut cursor),
        );
        let nbt = papokin_nbt::Nbt::read(&mut reader)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

        let data_version = nbt.get_int("DataVersion").unwrap_or(DATA_VERSION);
        let mut sections = FxHashMap::default();

        if let Some(sec_tag) = nbt.get_compound("Sections") {
            for (sec_key, tag) in &sec_tag.child_tags {
                if let papokin_nbt::tag::NbtTag::Compound(sec_comp) = tag {
                    let valid = sec_comp.get_byte("Valid").unwrap_or(1);
                    let mut records = Vec::new();
                    if let Some(papokin_nbt::tag::NbtTag::List(rec_list)) = sec_comp.get("Records")
                    {
                        for rec_t in rec_list {
                            if let papokin_nbt::tag::NbtTag::Compound(rc) = rec_t {
                                records.push(PoiEntry {
                                    x: rc.get_int("x").unwrap_or(0),
                                    y: rc.get_int("y").unwrap_or(0),
                                    z: rc.get_int("z").unwrap_or(0),
                                    poi_type: rc
                                        .get_string("type")
                                        .unwrap_or(POI_TYPE_NETHER_PORTAL)
                                        .to_string(),
                                    free_tickets: rc.get_int("free_tickets").unwrap_or(0),
                                });
                            }
                        }
                    }
                    sections.insert(sec_key.to_string(), PoiSectionData { valid, records });
                }
            }
        }

        Ok(PoiChunkData {
            data_version,
            sections,
        })
    }

    pub fn save(&mut self, path: &Path) -> std::io::Result<()> {
        if !self.dirty {
            return Ok(());
        }

        if self.entries.is_empty() {
            // 不保存空的 region，若文件存在则删除
            if path.exists() {
                std::fs::remove_file(path)?;
            }
            self.dirty = false;
            self.dirty_chunks.clear();
            return Ok(());
        }

        // 构建所有区块数据
        let mut chunk_data_map: FxHashMap<usize, Vec<u8>> = FxHashMap::default();

        // 收集所有包含条目的唯一区块
        let mut chunks_with_data: rustc_hash::FxHashSet<(i32, i32)> =
            rustc_hash::FxHashSet::default();
        for entry in self.entries.values() {
            chunks_with_data.insert((entry.x >> 4, entry.z >> 4));
        }

        for (chunk_x, chunk_z) in &chunks_with_data {
            if let Some(chunk_data) = self.get_chunk_data(*chunk_x, *chunk_z) {
                let compressed = Self::compress_chunk_data(&chunk_data)?;
                let index = Self::chunk_index(*chunk_x, *chunk_z);
                chunk_data_map.insert(index, compressed);
            }
        }

        // 构建 MCA 文件
        let mut location_table = [0u32; CHUNK_COUNT];
        let mut timestamp_table = [0u32; CHUNK_COUNT];
        let mut sector_data: Vec<Vec<u8>> = Vec::new();

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as u32);

        // 从头部之后开始（2 个扇区）
        let mut current_sector: u32 = 2;

        for index in 0..CHUNK_COUNT {
            if let Some(compressed) = chunk_data_map.get(&index) {
                // 计算所需的扇区数量
                let data_len = compressed.len() + 5; // 4 字节长度 + 1 字节压缩标志 + 数据
                let sector_count = data_len.div_ceil(SECTOR_SIZE) as u32;

                // 构建填充后的扇区数据
                let mut padded = Vec::with_capacity(sector_count as usize * SECTOR_SIZE);
                let length = (compressed.len() + 1) as u32; // 压缩字节 +1
                padded.extend_from_slice(&length.to_be_bytes());
                padded.push(COMPRESSION_ZLIB);
                padded.extend_from_slice(compressed);
                // 填充至扇区边界
                padded.resize(sector_count as usize * SECTOR_SIZE, 0);

                location_table[index] = (current_sector << 8) | sector_count;
                timestamp_table[index] = timestamp;
                sector_data.push(padded);

                current_sector += sector_count;
            }
        }

        // 在内存中组装整个文件并原子地替换进去，这样
        // 写入中途崩溃绝不会留下残缺的 POI 文件。
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file_bytes = Vec::with_capacity(HEADER_SIZE + sector_data.len() * SECTOR_SIZE);
        for loc in &location_table {
            file_bytes.extend_from_slice(&loc.to_be_bytes());
        }
        for ts in &timestamp_table {
            file_bytes.extend_from_slice(&ts.to_be_bytes());
        }
        for data in &sector_data {
            file_bytes.extend_from_slice(data);
        }

        let tmp_path = path.with_extension("tmp_poi");
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(&file_bytes)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp_path, path)?;

        self.dirty = false;
        self.dirty_chunks.clear();
        Ok(())
    }

    pub fn load(path: &Path) -> std::io::Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }

        let file_data = std::fs::read(path)?;
        // 空文件视为尚未写入的 region；非空但不足文件头长度
        // 说明写入中途被截断，按损坏处理（调用方会先备份再重建）
        if file_data.is_empty() {
            return Ok(Self::new());
        }
        if file_data.len() < HEADER_SIZE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "POI 文件头不完整",
            ));
        }

        let mut region = Self::new();

        // 解析位置表
        for index in 0..CHUNK_COUNT {
            let offset = index * 4;
            let location = u32::from_be_bytes([
                file_data[offset],
                file_data[offset + 1],
                file_data[offset + 2],
                file_data[offset + 3],
            ]);

            let sector_offset = (location >> 8) as usize;
            let sector_count = (location & 0xFF) as usize;

            if sector_offset == 0 || sector_count == 0 {
                continue;
            }

            let byte_offset = sector_offset * SECTOR_SIZE;
            let byte_end = byte_offset + sector_count * SECTOR_SIZE;

            if byte_end > file_data.len() {
                continue;
            }

            // 读取区块数据
            let chunk_bytes = &file_data[byte_offset..byte_end];
            if chunk_bytes.len() < 5 {
                continue;
            }

            let length = u32::from_be_bytes([
                chunk_bytes[0],
                chunk_bytes[1],
                chunk_bytes[2],
                chunk_bytes[3],
            ]) as usize;
            let compression = chunk_bytes[4];

            if compression != COMPRESSION_ZLIB || length < 1 || length > chunk_bytes.len() - 4 {
                continue;
            }

            let compressed = &chunk_bytes[5..5 + length - 1];

            match Self::decompress_chunk_data(compressed) {
                Ok(chunk_data) => {
                    for (_section_key, section) in chunk_data.sections {
                        for entry in section.records {
                            let key = (entry.x, entry.y, entry.z);
                            region.entries.insert(key, entry);
                        }
                    }
                }
                Err(e) => {
                    warn!("解析索引 {index} 处的 POI 区块失败：{e}");
                }
            }
        }

        region.dirty = false;
        Ok(region)
    }
}

/// 使用 MCA 格式的基于区域的 POI 存储
pub struct PoiStorage {
    /// poi 文件夹的路径
    folder: PathBuf,
    /// 已加载的区域，以（`region_x`, `region_z`）为键
    regions: FxHashMap<(i32, i32), PoiRegion>,
}

impl PoiStorage {
    #[must_use]
    pub fn new(poi_folder: PathBuf) -> Self {
        Self {
            folder: poi_folder,
            regions: FxHashMap::default(),
        }
    }

    const fn region_coords(pos: &BlockPos) -> (i32, i32) {
        let chunk_x = pos.0.x >> 4;
        let chunk_z = pos.0.z >> 4;
        (chunk_x >> 5, chunk_z >> 5)
    }

    fn region_path(&self, rx: i32, rz: i32) -> PathBuf {
        self.folder.join(format!("r.{rx}.{rz}.mca"))
    }

    fn get_or_load_region(&mut self, rx: i32, rz: i32) -> &mut PoiRegion {
        let path = self.region_path(rx, rz);
        self.regions.entry((rx, rz)).or_insert_with(|| {
            match PoiRegion::load(&path) {
                Ok(region) => region,
                Err(e) => {
                    warn!("加载 POI 区域 {} 失败：{e}", path.display());
                    // 加载失败时必须先把原文件改名备份：否则插入的
                    // 空 region 会在后续 add 触发保存时，用只含新条目
                    // 的数据覆盖整个 .mca，存量 POI 将全部静默丢失
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs());
                    let backup = path.with_extension(format!("corrupt-{timestamp}"));
                    if let Err(backup_err) = std::fs::rename(&path, &backup) {
                        warn!(
                            "备份损坏的 POI 文件到 {} 失败：{backup_err}",
                            backup.display()
                        );
                    }
                    PoiRegion::new()
                }
            }
        })
    }

    pub fn add(&mut self, pos: BlockPos, poi_type: &str) {
        self.add_with_free_tickets(pos, poi_type, 0);
    }

    pub fn add_with_free_tickets(&mut self, pos: BlockPos, poi_type: &str, free_tickets: i32) {
        let (rx, rz) = Self::region_coords(&pos);
        let region = self.get_or_load_region(rx, rz);
        region.add(PoiEntry {
            x: pos.0.x,
            y: pos.0.y,
            z: pos.0.z,
            poi_type: poi_type.to_string(),
            free_tickets,
        });
    }

    pub fn add_portal(&mut self, pos: BlockPos) {
        self.add(pos, POI_TYPE_NETHER_PORTAL);
    }

    pub fn remove(&mut self, pos: &BlockPos) -> bool {
        let (rx, rz) = Self::region_coords(pos);
        let region = self.get_or_load_region(rx, rz);
        region.remove(pos)
    }

    /// 获取方形半径内的所有 POI 位置（用于传送门搜索）
    #[expect(clippy::similar_names)]
    pub fn get_in_square(
        &mut self,
        center: BlockPos,
        radius: i32,
        poi_type: Option<&str>,
    ) -> Vec<BlockPos> {
        let min_x = center.0.x - radius;
        let max_x = center.0.x + radius;
        let min_z = center.0.z - radius;
        let max_z = center.0.z + radius;

        // 计算需要检查哪些区域
        let min_rx = (min_x >> 4) >> 5;
        let max_rx = (max_x >> 4) >> 5;
        let min_rz = (min_z >> 4) >> 5;
        let max_rz = (max_z >> 4) >> 5;

        let mut results = Vec::new();

        for rx in min_rx..=max_rx {
            for rz in min_rz..=max_rz {
                let region = self.get_or_load_region(rx, rz);
                for entry in region.get_all() {
                    if let Some(filter_type) = poi_type
                        && entry.poi_type != filter_type
                    {
                        continue;
                    }

                    let dx = (entry.x - center.0.x).abs();
                    let dz = (entry.z - center.0.z).abs();
                    if dx <= radius && dz <= radius {
                        results.push(entry.pos());
                    }
                }
            }
        }

        results
    }

    /// 查找类型匹配 `matches` 的最近 POI，并考虑
    /// 在 `center` 沿 x/z 轴 `radius` 方块范围内的条目（类似
    /// 原版的 `PoiManager.findClosestWithType`：先做切比雪夫方形收集
    /// 随后选取最小的 3D 平方距离）。
    ///
    /// 返回条目的位置及其类型。
    pub fn find_closest_matching(
        &mut self,
        center: BlockPos,
        radius: i32,
        matches: impl Fn(&str) -> bool,
    ) -> Option<(BlockPos, String)> {
        let min_rx = ((center.0.x - radius) >> 4) >> 5;
        let max_rx = ((center.0.x + radius) >> 4) >> 5;
        let min_rz = ((center.0.z - radius) >> 4) >> 5;
        let max_rz = ((center.0.z + radius) >> 4) >> 5;

        let mut best: Option<(BlockPos, String, i64)> = None;

        for rx in min_rx..=max_rx {
            for rz in min_rz..=max_rz {
                let region = self.get_or_load_region(rx, rz);
                for entry in region.get_all() {
                    if (entry.x - center.0.x).abs() > radius
                        || (entry.z - center.0.z).abs() > radius
                        || !matches(&entry.poi_type)
                    {
                        continue;
                    }

                    let dx = i64::from(entry.x - center.0.x);
                    let dy = i64::from(entry.y - center.0.y);
                    let dz = i64::from(entry.z - center.0.z);
                    let distance_sq = dx * dx + dy * dy + dz * dz;

                    if best.as_ref().is_none_or(|(_, _, d)| distance_sq < *d) {
                        best = Some((entry.pos(), entry.poi_type.clone(), distance_sq));
                    }
                }
            }
        }

        best.map(|(pos, poi_type, _)| (pos, poi_type))
    }

    pub fn save_all(&mut self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.folder)?;

        let mut saved = 0;
        for ((rx, rz), region) in &mut self.regions {
            if region.is_dirty() {
                let path = self.folder.join(format!("r.{rx}.{rz}.mca"));
                region.save(&path)?;
                saved += 1;
            }
        }

        if saved > 0 {
            info!("已保存 {saved} 个 POI 区域");
        }
        Ok(())
    }

    /// 获取已加载 Region 的数量
    #[must_use]
    pub fn loaded_region_count(&self) -> usize {
        self.regions.len()
    }

    /// 获取所有已加载 Region 中的 POI 总数
    #[must_use]
    pub fn total_poi_count(&self) -> usize {
        self.regions.values().map(|r| r.get_all().len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poi_entry() {
        let entry = PoiEntry::new_portal(BlockPos(Vector3::new(100, 64, 200)));
        assert_eq!(entry.x, 100);
        assert_eq!(entry.y, 64);
        assert_eq!(entry.z, 200);
        assert_eq!(entry.poi_type, POI_TYPE_NETHER_PORTAL);
    }

    #[test]
    fn poi_region() {
        let mut region = PoiRegion::new();
        region.add(PoiEntry::new_portal(BlockPos(Vector3::new(100, 64, 200))));
        region.add(PoiEntry::new_portal(BlockPos(Vector3::new(101, 64, 200))));

        assert_eq!(region.get_all().len(), 2);
        assert!(region.is_dirty());

        region.remove(&BlockPos(Vector3::new(100, 64, 200)));
        assert_eq!(region.get_all().len(), 1);
    }

    #[test]
    fn poi_find_closest_matching() {
        let mut storage = PoiStorage::new(std::env::temp_dir().join("papokin_poi_closest_test"));

        storage.add_portal(BlockPos(Vector3::new(100, 64, 100)));
        storage.add_portal(BlockPos(Vector3::new(120, 64, 100)));
        storage.add(BlockPos(Vector3::new(101, 64, 100)), "minecraft:home");

        let center = BlockPos(Vector3::new(105, 64, 100));
        let (pos, poi_type) = storage
            .find_closest_matching(center, 256, |t| t == POI_TYPE_NETHER_PORTAL)
            .unwrap();
        assert_eq!(pos, BlockPos(Vector3::new(100, 64, 100)));
        assert_eq!(poi_type, POI_TYPE_NETHER_PORTAL);

        // 整体最近的那一个会忽略上述类型过滤不匹配的情况。
        let (pos, poi_type) = storage
            .find_closest_matching(center, 256, |_| true)
            .unwrap();
        assert_eq!(pos, BlockPos(Vector3::new(101, 64, 100)));
        assert_eq!(poi_type, "minecraft:home");

        assert!(
            storage
                .find_closest_matching(center, 256, |t| t == "minecraft:lodestone")
                .is_none()
        );
        // 超出水平范围。
        assert!(
            storage
                .find_closest_matching(BlockPos(Vector3::new(1000, 64, 100)), 16, |_| true)
                .is_none()
        );
    }

    /// 损坏的 POI 文件必须先备份再重建：否则空 region 会在
    /// 后续保存时覆盖 .mca，存量 POI 全部静默丢失
    #[test]
    fn corrupt_poi_file_is_backed_up_not_overwritten() {
        let dir = std::env::temp_dir().join("papokin_poi_corrupt_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let path = dir.join("r.0.0.mca");
        // 长度不足文件头的残缺文件（写入中途被截断的形态）
        std::fs::write(&path, b"truncated-garbage").unwrap();

        let mut storage = PoiStorage::new(dir.clone());
        // 触发该 region 加载：失败后原文件应被改名为备份
        storage.add_portal(BlockPos(Vector3::new(100, 64, 100)));
        assert!(!path.exists(), "损坏文件应已被改名备份");

        let backups: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("r.0.0.corrupt-"))
            })
            .collect();
        assert_eq!(backups.len(), 1, "应恰好生成一个备份文件");
        assert_eq!(
            std::fs::read(&backups[0]).unwrap(),
            b"truncated-garbage",
            "备份必须保留原始字节"
        );

        // 新数据可以正常累积并保存，且不会触碰备份
        storage.save_all().unwrap();
        assert!(path.exists(), "重建后的 region 应已保存");
        assert!(backups[0].exists(), "备份文件必须保留");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn poi_storage_mca() {
        let dir = std::env::temp_dir().join("papokin_poi_mca_test");
        let _ = std::fs::remove_dir_all(&dir);

        let mut storage = PoiStorage::new(dir.join("poi"));

        storage.add_portal(BlockPos(Vector3::new(100, 64, 100)));
        storage.add_portal(BlockPos(Vector3::new(110, 64, 100)));
        storage.add_portal(BlockPos(Vector3::new(1000, 64, 1000))); // 不同的 region

        let results = storage.get_in_square(
            BlockPos(Vector3::new(105, 64, 100)),
            16,
            Some(POI_TYPE_NETHER_PORTAL),
        );
        assert_eq!(results.len(), 2);

        storage.save_all().unwrap();

        // 校验 .mca 文件已创建
        let mca_path = dir.join("poi").join("r.0.0.mca");
        assert!(mca_path.exists());

        // 重载并校验
        let mut storage2 = PoiStorage::new(dir.join("poi"));
        let results2 = storage2.get_in_square(
            BlockPos(Vector3::new(105, 64, 100)),
            16,
            Some(POI_TYPE_NETHER_PORTAL),
        );
        assert_eq!(results2.len(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
