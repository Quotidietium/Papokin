use std::{
    path::PathBuf,
    str::FromStr,
    sync::{
        RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use bytes::Bytes;
use papokin_data::{Block, BlockStateId, chunk::ChunkStatus, fluid::Fluid};
use papokin_nbt::compound::NbtCompound;
use papokin_util::resource_location::{FromResourceLocation, ResourceLocation, ToResourceLocation};
use rustc_hash::FxHashMap;

use crate::{
    chunk::{
        ChunkEntityData, ChunkReadingError, ChunkSerializingError,
        format::anvil::{SingleChunkDataSerializer, WORLD_DATA_VERSION},
        io::{Dirtiable, file_manager::PathFromLevelFolder},
    },
    generation::section_coords,
    level::LevelFolder,
    tick::{ScheduledTick, TickPriority, scheduler::ChunkTickScheduler},
};
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector2::Vector2;

use super::{
    ChunkData, ChunkHeightmaps, ChunkLight, ChunkParsingError, ChunkSections,
    palette::{BiomePalette, BlockPalette},
};
pub mod anvil;
pub mod linear;
pub mod pump;

/// 区块 NBT 中由 Pumpkin 自行读取并重写的根键。
///
/// 其余所有内容均通过其原样保留
/// [`crate::chunk::PreservedChunkData`]，并原样写回。
/// `Heightmaps` 被有意省略：完整的原始复合标签会保留
/// 而托管条目会在保存时合并回其中。
const MANAGED_ROOT_KEYS: &[&str] = &[
    "DataVersion",
    "xPos",
    "zPos",
    "yPos",
    "Status",
    "sections",
    "block_ticks",
    "fluid_ticks",
    "block_entities",
    "isLightOn",
    "InhabitedTime",
    "PumpkinCustomData",
    "BukkitValues",
];

/// 区块状态的规范 NBT 名称。
const fn status_to_str(status: ChunkStatus) -> &'static str {
    match status {
        ChunkStatus::Empty => "minecraft:empty",
        ChunkStatus::StructureStarts => "minecraft:structure_starts",
        ChunkStatus::StructureReferences => "minecraft:structure_references",
        ChunkStatus::Biomes => "minecraft:biomes",
        ChunkStatus::Terrain => "minecraft:terrain",
        ChunkStatus::Features => "minecraft:features",
        ChunkStatus::InitializeLight => "minecraft:initialize_light",
        ChunkStatus::Light => "minecraft:light",
        ChunkStatus::Spawn => "minecraft:spawn",
        ChunkStatus::Full => "minecraft:full",
    }
}

impl SingleChunkDataSerializer for ChunkData {
    #[inline]
    fn from_bytes(bytes: &Bytes, pos: Vector2<i32>) -> Result<Self, ChunkReadingError> {
        Self::internal_from_bytes(bytes, pos).map_err(ChunkReadingError::ParsingError)
    }

    #[inline]
    fn to_bytes(&self) -> Result<Bytes, ChunkSerializingError> {
        Ok(self.internal_to_bytes())
    }

    #[inline]
    fn position(&self) -> (i32, i32) {
        (self.x, self.z)
    }
}

impl PathFromLevelFolder for ChunkData {
    #[inline]
    fn file_path(folder: &LevelFolder, file_name: &str) -> PathBuf {
        folder.region_folder.join(file_name)
    }
}

impl Dirtiable for ChunkData {
    #[inline]
    fn mark_dirty(&self, flag: bool) {
        self.dirty.store(flag, Ordering::Relaxed);
    }

    #[inline]
    fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }
}

fn extract_u16_array(tag: &papokin_nbt::tag::NbtTag) -> Option<Box<[BlockStateId]>> {
    match tag {
        papokin_nbt::tag::NbtTag::IntArray(arr) => Some(
            arr.iter()
                .map(|&x| BlockStateId::new_or_air(x as u16))
                .collect(),
        ),
        papokin_nbt::tag::NbtTag::ByteArray(arr) => Some(
            arr.iter()
                .map(|&x| BlockStateId::new_or_air(x as u16))
                .collect(),
        ),
        papokin_nbt::tag::NbtTag::LongArray(arr) => Some(
            arr.iter()
                .map(|&x| BlockStateId::new_or_air(x as u16))
                .collect(),
        ),
        papokin_nbt::tag::NbtTag::List(list) => {
            let ids: Box<[BlockStateId]> = list
                .iter()
                .map(|t| match t {
                    papokin_nbt::tag::NbtTag::Int(x) => BlockStateId::new_or_air(*x as u16),
                    papokin_nbt::tag::NbtTag::Short(x) => BlockStateId::new_or_air(*x as u16),
                    papokin_nbt::tag::NbtTag::Byte(x) => BlockStateId::new_or_air(*x as u16),
                    papokin_nbt::tag::NbtTag::Long(x) => BlockStateId::new_or_air(*x as u16),
                    papokin_nbt::tag::NbtTag::Compound(compound) => {
                        if let Ok(entry) =
                            crate::generation::structure::template::PaletteEntry::from_nbt_compound(
                                compound,
                            )
                            && let Some(state) =
                                crate::generation::structure::template::BlockStateResolver::resolve_simple(
                                    &entry,
                                )
                        {
                            return state.id;
                        }
                        BlockStateId::AIR
                    }
                    _ => BlockStateId::AIR,
                })
                .collect();
            Some(ids)
        }
        _ => None,
    }
}

fn extract_u8_array(tag: &papokin_nbt::tag::NbtTag) -> Option<Box<[u8]>> {
    match tag {
        papokin_nbt::tag::NbtTag::ByteArray(arr) => Some(arr.iter().map(|&x| x as u8).collect()),
        papokin_nbt::tag::NbtTag::IntArray(arr) => Some(arr.iter().map(|&x| x as u8).collect()),
        papokin_nbt::tag::NbtTag::List(list) => {
            let bytes: Box<[u8]> = list
                .iter()
                .map(|t| match t {
                    papokin_nbt::tag::NbtTag::Byte(x) => *x as u8,
                    papokin_nbt::tag::NbtTag::Int(x) => *x as u8,
                    papokin_nbt::tag::NbtTag::Short(x) => *x as u8,
                    papokin_nbt::tag::NbtTag::String(s) => {
                        let name = s.strip_prefix("minecraft:").unwrap_or(s);
                        papokin_data::biome::Biome::from_name(name).map_or(0, |b| b.id)
                    }
                    _ => 0,
                })
                .collect();
            Some(bytes)
        }
        _ => None,
    }
}

fn parse_scheduled_tick<T>(nbt: &papokin_nbt::compound::NbtCompound) -> Option<ScheduledTick<T>>
where
    T: FromResourceLocation,
{
    let x = nbt.get_int("x")?;
    let y = nbt.get_int("y")?;
    let z = nbt.get_int("z")?;
    let delay = nbt.get_int("t")? as u8;
    let priority = TickPriority::try_from(nbt.get_int("p")?).ok()?;
    let res_loc_str = nbt.get_string("i")?;
    let res_loc = ResourceLocation::from_str(res_loc_str).ok()?;
    let value = T::from_resource_location(&res_loc)?;
    Some(ScheduledTick {
        delay,
        priority,
        position: BlockPos::new(x, y, z),
        value,
    })
}

impl ChunkData {
    #[allow(clippy::too_many_lines)]
    pub fn internal_from_bytes(
        chunk_data: &[u8],
        position: Vector2<i32>,
    ) -> Result<Self, ChunkParsingError> {
        let is_named = chunk_data.len() >= 3
            && chunk_data[0] == 0x0a
            && chunk_data[1] == 0x00
            && chunk_data[2] == 0x00;

        let mut cursor = std::io::Cursor::new(chunk_data);
        let mut reader = papokin_nbt::deserializer::NbtReadHelperJava::new(&mut cursor);
        let nbt = if is_named {
            papokin_nbt::Nbt::read(&mut reader)
        } else {
            papokin_nbt::Nbt::read_unnamed(&mut reader)
        }
        .map_err(|e| ChunkParsingError::ErrorDeserializingChunk(e.to_string()))?;

        let root_tag = nbt.root_tag;

        let x_pos = root_tag.get_int("xPos").ok_or_else(|| {
            ChunkParsingError::ErrorDeserializingChunk("Missing xPos".to_string())
        })?;
        let z_pos = root_tag.get_int("zPos").ok_or_else(|| {
            ChunkParsingError::ErrorDeserializingChunk("Missing zPos".to_string())
        })?;

        if x_pos != position.x || z_pos != position.y {
            return Err(ChunkParsingError::ErrorDeserializingChunk(format!(
                "Expected data for chunk {},{} but got it for {},{}!",
                position.x, position.y, x_pos, z_pos,
            )));
        }

        let min_y_section = root_tag.get_int("yPos").ok_or_else(|| {
            ChunkParsingError::ErrorDeserializingChunk("Missing yPos".to_string())
        })?;

        let mut max_y_section = min_y_section as i8;
        if let Some(sections_list) = root_tag.get_list("sections") {
            for section_tag in sections_list {
                if let papokin_nbt::tag::NbtTag::Compound(section_compound) = section_tag {
                    let y = section_compound.get_byte("Y").unwrap_or(0);
                    if y > max_y_section {
                        max_y_section = y;
                    }
                }
            }
        }

        let section_count = (max_y_section as i32 - min_y_section + 1).max(0) as usize;
        let mut block_lights = vec![LightContainer::Empty(0); section_count];
        let mut sky_lights = vec![LightContainer::Empty(0); section_count];
        let mut block_palettes = vec![BlockPalette::default(); section_count];
        let mut biome_palettes = vec![BiomePalette::default(); section_count];

        if let Some(sections_list) = root_tag.get_list("sections") {
            for section_tag in sections_list {
                if let papokin_nbt::tag::NbtTag::Compound(section_compound) = section_tag {
                    let y = section_compound.get_byte("Y").unwrap_or(0);
                    let index = (y as i32 - min_y_section) as usize;
                    if index >= section_count {
                        continue;
                    }

                    let block_light = section_compound
                        .get("BlockLight")
                        .and_then(|tag| tag.extract_byte_array())
                        .map(|arr| {
                            // SAFETY: `arr` 是 `i8` 切片（`&[i8]`）。`u8` 与 `i8` 具有相同的内存布局、对齐（1 字节）和生命周期。
                            unsafe {
                                Box::from(std::slice::from_raw_parts(
                                    arr.as_ptr().cast::<u8>(),
                                    arr.len(),
                                ))
                            }
                        });

                    let sky_light = section_compound
                        .get("SkyLight")
                        .and_then(|tag| tag.extract_byte_array())
                        .map(|arr| {
                            // SAFETY: `arr` 是 `i8` 切片（`&[i8]`）。`u8` 与 `i8` 具有相同的内存布局、对齐（1 字节）和生命周期。
                            unsafe {
                                Box::from(std::slice::from_raw_parts(
                                    arr.as_ptr().cast::<u8>(),
                                    arr.len(),
                                ))
                            }
                        });

                    block_lights[index] =
                        block_light.map_or(LightContainer::Empty(0), LightContainer::Full);
                    sky_lights[index] =
                        sky_light.map_or(LightContainer::Empty(0), LightContainer::Full);

                    if let Some(bs_compound) = section_compound.get_compound("block_states") {
                        let data = bs_compound
                            .get_long_array("data")
                            .map(|arr| arr.to_vec().into_boxed_slice());
                        let palette = bs_compound
                            .get("palette")
                            .and_then(extract_u16_array)
                            .unwrap_or_else(|| vec![BlockStateId::AIR].into_boxed_slice());

                        block_palettes[index] =
                            BlockPalette::from_disk_nbt(ChunkSectionBlockStates { data, palette });
                    } else {
                        block_palettes[index] = BlockPalette::default();
                    }

                    if let Some(b_compound) = section_compound.get_compound("biomes") {
                        let data = b_compound
                            .get_long_array("data")
                            .map(|arr| arr.to_vec().into_boxed_slice());
                        let palette = b_compound
                            .get("palette")
                            .and_then(extract_u8_array)
                            .unwrap_or_else(|| vec![0].into_boxed_slice());

                        biome_palettes[index] =
                            BiomePalette::from_disk_nbt(ChunkSectionBiomes { data, palette });
                    } else {
                        biome_palettes[index] = BiomePalette::default();
                    }
                }
            }
        }

        // 组装 LightEngine
        let light_engine = ChunkLight {
            block_light: block_lights.into_boxed_slice(),
            sky_light: sky_lights.into_boxed_slice(),
        };

        // 组装 ChunkSections
        let min_y = section_coords::section_to_block(min_y_section);
        let (random_tick_sections, randomly_ticking_mask) =
            ChunkSections::build_random_tick_sections_cache(&block_palettes);
        let section = ChunkSections {
            count: block_palettes.len(),
            block_sections: RwLock::new(block_palettes.into_boxed_slice()),
            random_tick_sections: RwLock::new(random_tick_sections),
            randomly_ticking_mask: std::sync::atomic::AtomicU32::new(randomly_ticking_mask),
            biome_sections: RwLock::new(biome_palettes.into_boxed_slice()),
            min_y,
        };

        let heightmaps = root_tag.get_compound("Heightmaps").map_or(
            ChunkHeightmaps {
                world_surface: None,
                motion_blocking: None,
                motion_blocking_no_leaves: None,
            },
            |h_compound| ChunkHeightmaps {
                world_surface: h_compound
                    .get_long_array("WORLD_SURFACE")
                    .map(|a| a.to_vec().into_boxed_slice()),
                motion_blocking: h_compound
                    .get_long_array("MOTION_BLOCKING")
                    .map(|a| a.to_vec().into_boxed_slice()),
                motion_blocking_no_leaves: h_compound
                    .get_long_array("MOTION_BLOCKING_NO_LEAVES")
                    .map(|a| a.to_vec().into_boxed_slice()),
            },
        );
        let mut block_ticks = Vec::new();
        if let Some(list) = root_tag.get_list("block_ticks") {
            for tag in list {
                if let papokin_nbt::tag::NbtTag::Compound(compound) = tag
                    && let Some(tick) = parse_scheduled_tick::<&'static Block>(compound)
                {
                    block_ticks.push(tick);
                }
            }
        }

        let mut fluid_ticks = Vec::new();
        if let Some(list) = root_tag.get_list("fluid_ticks") {
            for tag in list {
                if let papokin_nbt::tag::NbtTag::Compound(compound) = tag
                    && let Some(tick) = parse_scheduled_tick::<&'static Fluid>(compound)
                {
                    fluid_ticks.push(tick);
                }
            }
        }

        let mut block_entities = FxHashMap::default();
        if let Some(list) = root_tag.get_list("block_entities") {
            for tag in list {
                if let papokin_nbt::tag::NbtTag::Compound(nbt) = tag
                    && let Some(x) = nbt.get_int("x")
                    && let Some(y) = nbt.get_int("y")
                    && let Some(z) = nbt.get_int("z")
                {
                    block_entities.insert(BlockPos::new(x, y, z), nbt.clone());
                }
            }
        }

        let light_correct = root_tag.get_bool("isLightOn").unwrap_or(false);

        let status_str = root_tag.get_string("Status").unwrap_or("minecraft:empty");
        let status = match status_str {
            "minecraft:structure_starts" => ChunkStatus::StructureStarts,
            "minecraft:structure_references" => ChunkStatus::StructureReferences,
            "minecraft:biomes" => ChunkStatus::Biomes,
            "minecraft:terrain" | "minecraft:noise" | "minecraft:surface" | "minecraft:carvers" => {
                ChunkStatus::Terrain
            }
            "minecraft:features" => ChunkStatus::Features,
            "minecraft:initialize_light" => ChunkStatus::InitializeLight,
            "minecraft:light" => ChunkStatus::Light,
            "minecraft:spawn" => ChunkStatus::Spawn,
            "minecraft:full" => ChunkStatus::Full,
            _ => ChunkStatus::Empty,
        };

        let custom_data = root_tag
            .get_compound("PumpkinCustomData")
            .or_else(|| root_tag.get_compound("BukkitValues"))
            .cloned()
            .unwrap_or_default();
        // Paper/Papo 世界将持久化数据存储在 `BukkitValues` 下；
        // 必须以相同名称写回，否则 Papo 会丢失它。
        let custom_data_tag = if root_tag.get_compound("PumpkinCustomData").is_some() {
            "PumpkinCustomData"
        } else if root_tag.get_compound("BukkitValues").is_some() {
            "BukkitValues"
        } else {
            "PumpkinCustomData"
        };

        // Pumpkin 不管理的每个根键都逐字保留，并且
        // 原样写回，因此保存永远不会丢弃外来的区块数据
        // （structures、blending_data、LastUpdate、未建模的高度图……）。
        let mut preserved_fields = NbtCompound::new();
        for (key, tag) in &root_tag.child_tags {
            if !MANAGED_ROOT_KEYS.contains(&&**key) {
                preserved_fields.child_tags.insert(key.clone(), tag.clone());
            }
        }

        // 旧版状态名（noise/surface/carvers）都解析为 `Terrain`；
        // 记住要写回的精确字符串，同时状态处于
        // 保持不变。
        let original_status =
            (status_str != status_to_str(status)).then(|| (status_str.to_string(), status));

        let preserved_data = (!preserved_fields.is_empty()
            || original_status.is_some()
            || custom_data_tag != "PumpkinCustomData")
            .then_some(super::PreservedChunkData {
                fields: preserved_fields,
                custom_data_tag,
                original_status,
            });

        Ok(Self {
            section,
            heightmap: std::sync::Mutex::new(heightmaps),
            x: position.x,
            z: position.y,
            // 此区块是从磁盘读取的，因此尚未被修改
            dirty: AtomicBool::new(false),
            block_ticks: ChunkTickScheduler::from_iter(block_ticks),
            fluid_ticks: ChunkTickScheduler::from_iter(fluid_ticks),
            pending_block_entities: std::sync::Mutex::new(block_entities),
            light_engine: std::sync::Mutex::new(light_engine),
            light_populated: AtomicBool::new(light_correct),
            status,
            blending_data: None,
            inhabited_time: AtomicU64::new(root_tag.get_long("InhabitedTime").unwrap_or(0) as u64),
            custom_data: std::sync::Mutex::new(custom_data),
            preserved_data: std::sync::Mutex::new(preserved_data),
        })
    }

    #[allow(clippy::too_many_lines)]
    fn internal_to_bytes(&self) -> Bytes {
        use papokin_nbt::tag::NbtTag;

        fn extract_light_ref(light: Option<&LightContainer>) -> Option<&[u8]> {
            match light {
                Some(LightContainer::Full(data)) => Some(data.as_ref()),
                _ => None,
            }
        }

        let is_light_correct = self
            .light_populated
            .load(std::sync::atomic::Ordering::Relaxed);

        let block_entities_nbt = {
            let entities_guard = self
                .pending_block_entities
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            entities_guard.values().cloned().collect::<Vec<_>>()
        };

        let light_lock = self
            .light_engine
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let heightmap_lock = self
            .heightmap
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let block_lock = self
            .section
            .block_sections
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let biome_lock = self
            .section
            .biome_sections
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let min_section_y = (self.section.min_y >> 4) as i8;

        // 从保留的外部字段开始，使它们能在保存后存活；
        // 之后每个受管键都会被放入并覆盖过时副本。
        let preserved = self
            .preserved_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut root_compound = preserved
            .as_ref()
            .map_or_else(NbtCompound::new, |p| p.fields.clone());
        root_compound.put_int("DataVersion", WORLD_DATA_VERSION);
        root_compound.put_int("xPos", self.x);
        root_compound.put_int("zPos", self.z);
        root_compound.put_int("yPos", section_coords::block_to_section(self.section.min_y));

        // 保留旧版状态名的原始拼写（例如
        // `minecraft:noise`），而状态尚未推进。
        let status_str = preserved
            .as_ref()
            .and_then(|p| p.original_status.as_ref())
            .filter(|(_, original_status)| *original_status == self.status)
            .map_or_else(
                || status_to_str(self.status).to_string(),
                |(original, _)| original.clone(),
            );
        root_compound.put_string("Status", status_str);

        // 将受管理的高度图合并到保留的复合标签中，保持
        // 未建模的条目（OCEAN_FLOOR、WORLD_SURFACE_WG 等）保持原样。
        let mut heightmaps_compound = match root_compound.child_tags.remove("Heightmaps") {
            Some(papokin_nbt::tag::NbtTag::Compound(compound)) => compound,
            _ => NbtCompound::new(),
        };
        if let Some(ref arr) = heightmap_lock.world_surface {
            heightmaps_compound.put("WORLD_SURFACE", NbtTag::LongArray(arr.to_vec()));
        }
        if let Some(ref arr) = heightmap_lock.motion_blocking {
            heightmaps_compound.put("MOTION_BLOCKING", NbtTag::LongArray(arr.to_vec()));
        }
        if let Some(ref arr) = heightmap_lock.motion_blocking_no_leaves {
            heightmaps_compound.put("MOTION_BLOCKING_NO_LEAVES", NbtTag::LongArray(arr.to_vec()));
        }
        root_compound.put_compound("Heightmaps", heightmaps_compound);

        let mut sections_list = Vec::new();
        for i in 0..self.section.count {
            let mut section_comp = NbtCompound::new();
            let y_val = i as i8 + min_section_y;
            section_comp.put_byte("Y", y_val);

            // 方块状态（block_states）
            let block_states_nbt = block_lock[i].to_disk_nbt();
            let mut bs_comp = NbtCompound::new();
            if let Some(ref data_arr) = block_states_nbt.data {
                bs_comp.put("data", NbtTag::LongArray(data_arr.to_vec()));
            }
            let palette_tags: Vec<NbtTag> = block_states_nbt
                .palette
                .iter()
                .map(|&id| {
                    let block = Block::from_state_id(id);
                    let mut comp = NbtCompound::new();
                    let name = if block.name.starts_with("minecraft:") {
                        block.name.to_string()
                    } else {
                        format!("minecraft:{}", block.name)
                    };
                    comp.put_string("Name", name);
                    if let Some(props) = block.properties(id) {
                        let prop_vec = props.to_props();
                        if !prop_vec.is_empty() {
                            let mut props_comp = NbtCompound::new();
                            for (k, v) in prop_vec {
                                props_comp.put_string(k, v.to_string());
                            }
                            comp.put_compound("Properties", props_comp);
                        }
                    }
                    NbtTag::Compound(comp)
                })
                .collect();
            bs_comp.put_list("palette", palette_tags);
            section_comp.put_compound("block_states", bs_comp);

            // 生物群系（biomes）
            let biomes_nbt = biome_lock[i].to_disk_nbt();
            let mut b_comp = NbtCompound::new();
            if let Some(ref data_arr) = biomes_nbt.data {
                b_comp.put("data", NbtTag::LongArray(data_arr.to_vec()));
            }
            let biome_palette_tags: Vec<NbtTag> = biomes_nbt
                .palette
                .iter()
                .map(|&val| {
                    let name = papokin_data::biome::Biome::from_id(val)
                        .map_or("plains", |b| b.registry_id);
                    let full_name = if name.starts_with("minecraft:") {
                        name.to_string()
                    } else {
                        format!("minecraft:{name}")
                    };
                    NbtTag::String(full_name.into())
                })
                .collect();
            b_comp.put_list("palette", biome_palette_tags);
            section_comp.put_compound("biomes", b_comp);

            // 方块光照（block_light）
            if let Some(light_data) = extract_light_ref(light_lock.block_light.get(i)) {
                let bytes: Box<[i8]> = light_data.iter().map(|&x| x as i8).collect();
                section_comp.put("BlockLight", NbtTag::ByteArray(bytes));
            }

            // sky_light
            if let Some(light_data) = extract_light_ref(light_lock.sky_light.get(i)) {
                let bytes: Box<[i8]> = light_data.iter().map(|&x| x as i8).collect();
                section_comp.put("SkyLight", NbtTag::ByteArray(bytes));
            }

            sections_list.push(NbtTag::Compound(section_comp));
        }
        root_compound.put_list("sections", sections_list);

        let mut block_ticks_list = Vec::new();
        for tick in self.block_ticks.to_vec() {
            let mut tick_comp = NbtCompound::new();
            tick_comp.put_int("x", tick.position.0.x);
            tick_comp.put_int("y", tick.position.0.y);
            tick_comp.put_int("z", tick.position.0.z);
            tick_comp.put_int("t", tick.delay as i32);
            tick_comp.put_int("p", tick.priority as i32);
            tick_comp.put_string("i", tick.value.to_resource_location());
            block_ticks_list.push(NbtTag::Compound(tick_comp));
        }
        root_compound.put_list("block_ticks", block_ticks_list);

        let mut fluid_ticks_list = Vec::new();
        for tick in self.fluid_ticks.to_vec() {
            let mut tick_comp = NbtCompound::new();
            tick_comp.put_int("x", tick.position.0.x);
            tick_comp.put_int("y", tick.position.0.y);
            tick_comp.put_int("z", tick.position.0.z);
            tick_comp.put_int("t", tick.delay as i32);
            tick_comp.put_int("p", tick.priority as i32);
            tick_comp.put_string("i", tick.value.to_resource_location());
            fluid_ticks_list.push(NbtTag::Compound(tick_comp));
        }
        root_compound.put_list("fluid_ticks", fluid_ticks_list);

        let mut block_entities_list = Vec::new();
        for entity_comp in block_entities_nbt {
            block_entities_list.push(NbtTag::Compound(entity_comp));
        }
        root_compound.put_list("block_entities", block_entities_list);

        root_compound.put_bool("isLightOn", is_light_correct);
        root_compound.put_long(
            "InhabitedTime",
            self.inhabited_time.load(Ordering::Relaxed) as i64,
        );

        let custom_data = self
            .custom_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !custom_data.is_empty() {
            // 将自定义数据写回其读取时的标签下，以便
            // Paper/Papo 世界完整保留其 `BukkitValues` PDC。
            let tag_name = preserved
                .as_ref()
                .map_or("PumpkinCustomData", |p| p.custom_data_tag);
            root_compound.put_compound(tag_name, custom_data.clone());
        }

        let nbt = papokin_nbt::Nbt::from(root_compound);
        nbt.write()
    }

    pub fn set_custom_data(&self, namespace: &str, key: &str, value: papokin_nbt::tag::NbtTag) {
        let mut custom_data = self
            .custom_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let mut namespace_data = custom_data
            .child_tags
            .remove(namespace)
            .and_then(|tag| match tag {
                papokin_nbt::tag::NbtTag::Compound(compound) => Some(compound),
                _ => None,
            })
            .unwrap_or_default();

        namespace_data.child_tags.insert(key.into(), value);
        custom_data.child_tags.insert(
            namespace.into(),
            papokin_nbt::tag::NbtTag::Compound(namespace_data),
        );
        self.dirty.store(true, Ordering::Relaxed);
    }

    pub fn get_custom_data(&self, namespace: &str, key: &str) -> Option<papokin_nbt::tag::NbtTag> {
        let custom_data = self
            .custom_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        custom_data
            .get(namespace)?
            .extract_compound()?
            .get(key)
            .cloned()
    }

    pub fn remove_custom_data(&self, namespace: &str, key: &str) {
        let mut custom_data = self
            .custom_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let Some(papokin_nbt::tag::NbtTag::Compound(mut namespace_data)) =
            custom_data.child_tags.remove(namespace)
        else {
            return;
        };

        namespace_data.child_tags.remove(key);
        if !namespace_data.is_empty() {
            custom_data.child_tags.insert(
                namespace.into(),
                papokin_nbt::tag::NbtTag::Compound(namespace_data),
            );
        }
        self.dirty.store(true, Ordering::Relaxed);
    }

    pub fn has_custom_data(&self, namespace: &str, key: &str) -> bool {
        self.get_custom_data(namespace, key).is_some()
    }
}

impl PathFromLevelFolder for ChunkEntityData {
    #[inline]
    fn file_path(folder: &LevelFolder, file_name: &str) -> PathBuf {
        folder.entities_folder.join(file_name)
    }
}

impl Dirtiable for ChunkEntityData {
    #[inline]
    fn mark_dirty(&self, flag: bool) {
        self.dirty.store(flag, Ordering::Relaxed);
    }

    #[inline]
    fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }
}

impl SingleChunkDataSerializer for ChunkEntityData {
    #[inline]
    fn from_bytes(bytes: &Bytes, pos: Vector2<i32>) -> Result<Self, ChunkReadingError> {
        Self::internal_from_bytes(bytes, pos).map_err(ChunkReadingError::ParsingError)
    }

    #[inline]
    fn to_bytes(&self) -> Result<Bytes, ChunkSerializingError> {
        Ok(self.internal_to_bytes())
    }

    #[inline]
    fn position(&self) -> (i32, i32) {
        (self.x, self.z)
    }
}

impl ChunkEntityData {
    fn internal_from_bytes(
        chunk_data: &[u8],
        position: Vector2<i32>,
    ) -> Result<Self, ChunkParsingError> {
        let is_named = chunk_data.len() >= 3
            && chunk_data[0] == 0x0a
            && chunk_data[1] == 0x00
            && chunk_data[2] == 0x00;
        let mut cursor = std::io::Cursor::new(chunk_data);
        let mut reader = papokin_nbt::deserializer::NbtReadHelperJava::new(
            papokin_nbt::deserializer::NbtStreamReader(&mut cursor),
        );
        let nbt = if is_named {
            papokin_nbt::Nbt::read(&mut reader)
        } else {
            papokin_nbt::Nbt::read_unnamed(&mut reader)
        }
        .map_err(|e| ChunkParsingError::ErrorDeserializingChunk(e.to_string()))?;

        let pos_array = match (nbt.get_int("Position-X"), nbt.get_int("Position-Z")) {
            (Some(x), Some(z)) => [x, z],
            _ => {
                if let Some(papokin_nbt::tag::NbtTag::IntArray(pos)) = nbt.get("Position") {
                    if pos.len() >= 2 {
                        [pos[0], pos[1]]
                    } else {
                        [0, 0]
                    }
                } else {
                    [0, 0]
                }
            }
        };

        if pos_array[0] != position.x || pos_array[1] != position.y {
            return Err(ChunkParsingError::ErrorDeserializingChunk(format!(
                "Expected data for entity chunk {},{} but got it for {},{}!",
                position.x, position.y, pos_array[0], pos_array[1],
            )));
        }

        let entities = match nbt.get("Entities") {
            Some(papokin_nbt::tag::NbtTag::List(list)) => list
                .iter()
                .filter_map(|t| match t {
                    papokin_nbt::tag::NbtTag::Compound(c) => Some(c.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };

        Ok(Self {
            x: position.x,
            z: position.y,
            data: std::sync::Mutex::new(entities),
            live: AtomicBool::new(false),
            dirty: AtomicBool::new(false),
        })
    }

    fn internal_to_bytes(&self) -> Bytes {
        let mut root = NbtCompound::new();
        root.put_int("DataVersion", WORLD_DATA_VERSION);
        root.put(
            "Position",
            papokin_nbt::tag::NbtTag::IntArray(vec![self.x, self.z]),
        );
        let entities_tag: Vec<papokin_nbt::tag::NbtTag> = self
            .data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|c| papokin_nbt::tag::NbtTag::Compound(c.clone()))
            .collect();
        root.put_list("Entities", entities_tag);

        let nbt = papokin_nbt::Nbt::from(root);
        nbt.write()
    }
}

#[derive(Clone)]
pub struct ChunkSectionBiomes {
    pub(crate) data: Option<Box<[i64]>>,
    pub(crate) palette: Box<[u8]>,
}

#[derive(Clone)]
pub struct ChunkSectionBlockStates {
    pub(crate) data: Option<Box<[i64]>>,
    pub(crate) palette: Box<[BlockStateId]>,
}

#[derive(Debug, Clone)]
pub enum LightContainer {
    Empty(u8),
    Full(Box<[u8]>),
}

impl LightContainer {
    pub const DIM: usize = 16;
    pub const ARRAY_SIZE: usize = Self::DIM * Self::DIM * Self::DIM / 2;

    #[must_use]
    pub fn new_empty(default: u8) -> Self {
        assert!(default <= 15, "Default value must be between 0 and 15");
        Self::Empty(default)
    }

    #[must_use]
    pub fn new(data: Box<[u8]>) -> Self {
        assert!(
            data.len() == Self::ARRAY_SIZE,
            "Data length must be {}",
            Self::ARRAY_SIZE
        );
        Self::Full(data)
    }

    #[must_use]
    pub fn new_filled(default: u8) -> Self {
        assert!(default <= 15, "Default value must be between 0 and 15");
        let value = default << 4 | default;
        Self::Full([value; Self::ARRAY_SIZE].into())
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        matches!(self, Self::Empty(_))
    }

    #[inline]
    const fn index(x: usize, y: usize, z: usize) -> usize {
        y * 16 * 16 + z * 16 + x
    }

    #[inline]
    #[must_use]
    pub fn get(&self, x: usize, y: usize, z: usize) -> u8 {
        match self {
            Self::Full(data) => {
                let index = Self::index(x, y, z);
                (data[index >> 1] >> (4 * (index & 1))) & 0x0F
            }
            Self::Empty(default) => *default,
        }
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, value: u8) {
        match self {
            Self::Full(data) => {
                let index = Self::index(x, y, z);
                let shift = 4 * (index & 1);
                let mask = 0x0F << shift;
                data[index >> 1] = (data[index >> 1] & !mask) | (value << shift);
            }
            Self::Empty(default) => {
                if value != *default {
                    *self = Self::new_filled(*default);
                    self.set(x, y, z, value);
                }
            }
        }
    }

    #[inline]
    pub fn set_column_y_range(
        &mut self,
        x: usize,
        z: usize,
        y_start: usize,
        y_end: usize,
        value: u8,
    ) {
        if y_start >= y_end {
            return;
        }
        match self {
            Self::Full(data) => {
                let shift = 4 * (x & 1);
                let mask = 0x0F << shift;
                let val = (value & 0x0F) << shift;
                let mut byte_idx = (y_start * 256 + z * 16 + x) >> 1;
                for _ in y_start..y_end {
                    data[byte_idx] = (data[byte_idx] & !mask) | val;
                    byte_idx += 128;
                }
            }
            Self::Empty(default) => {
                if value != *default {
                    *self = Self::new_filled(*default);
                    self.set_column_y_range(x, z, y_start, y_end, value);
                }
            }
        }
    }

    #[inline]
    pub fn fill(&mut self, value: u8) {
        *self = Self::new_filled(value);
    }
}

impl Default for LightContainer {
    fn default() -> Self {
        Self::new_empty(15)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use papokin_data::Block;
    use papokin_nbt::compound::NbtCompound;
    use papokin_nbt::tag::NbtTag;

    #[test]
    fn extract_u16_array_from_vanilla_compound_palette() {
        let mut entry1 = NbtCompound::new();
        entry1.put_string("Name", "minecraft:stone".to_string());

        let mut entry2 = NbtCompound::new();
        entry2.put_string("Name", "minecraft:repeater".to_string());
        let mut props = NbtCompound::new();
        props.put_string("facing", "north".to_string());
        props.put_string("delay", "2".to_string());
        props.put_string("locked", "false".to_string());
        props.put_string("powered", "false".to_string());
        entry2.put_compound("Properties", props);

        let list_tag = NbtTag::List(vec![NbtTag::Compound(entry1), NbtTag::Compound(entry2)]);
        let result = extract_u16_array(&list_tag).expect("应能提取调色板");

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], Block::STONE.default_state.id);

        let repeater_state = Block::REPEATER
            .from_properties(&[
                ("facing", "north"),
                ("delay", "2"),
                ("locked", "false"),
                ("powered", "false"),
            ])
            .to_state_id(&Block::REPEATER);
        assert_eq!(result[1], repeater_state);
    }

    #[test]
    fn extract_u8_array_from_vanilla_string_palette() {
        let list_tag = NbtTag::List(vec![
            NbtTag::String("minecraft:plains".to_string().into()),
            NbtTag::String("minecraft:the_void".to_string().into()),
        ]);
        let result = extract_u8_array(&list_tag).expect("应能提取生物群系调色板");

        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0],
            papokin_data::biome::Biome::from_name("plains").unwrap().id
        );
        assert_eq!(
            result[1],
            papokin_data::biome::Biome::from_name("the_void")
                .unwrap()
                .id
        );
    }

    #[test]
    fn foreign_chunk_fields_survive_round_trip() {
        // 一个 Papo/Paper 风格的区块，携带 Pumpkin 无模型的键：
        // 它们在加载/保存往返之后必须逐字节一致。
        let mut root = NbtCompound::new();
        root.put_int("xPos", 5);
        root.put_int("zPos", -3);
        root.put_int("yPos", -4);
        // 解析为 `Terrain` 的旧版状态名。
        root.put_string("Status", "minecraft:noise".to_string());
        root.put_long("LastUpdate", 123_456);
        let mut structures = NbtCompound::new();
        let mut starts = NbtCompound::new();
        let mut fortress = NbtCompound::new();
        fortress.put_string("id", "minecraft:fortress".to_string());
        starts.put_compound("minecraft:fortress", fortress);
        structures.put_compound("starts", starts);
        root.put_compound("structures", structures);
        let mut blending = NbtCompound::new();
        blending.put_int("min_section_x", 5);
        root.put_compound("blending_data", blending);
        let mut heightmaps = NbtCompound::new();
        heightmaps.put("OCEAN_FLOOR", NbtTag::LongArray(vec![7; 37]));
        heightmaps.put("WORLD_SURFACE", NbtTag::LongArray(vec![9; 37]));
        root.put_compound("Heightmaps", heightmaps);
        let mut bukkit = NbtCompound::new();
        bukkit.put("papo-key", NbtTag::String("papo-value".to_string().into()));
        root.put_compound("BukkitValues", bukkit);
        root.put_long("InhabitedTime", 77);
        root.put_bool("isLightOn", true);

        let bytes = papokin_nbt::Nbt::from(root).write_unnamed();
        let chunk = ChunkData::internal_from_bytes(&bytes, Vector2::new(5, -3)).expect("解析");
        let out = chunk.internal_to_bytes();

        // `internal_to_bytes` 写入的是一个名称为空的具名根标签。
        let mut cursor = std::io::Cursor::new(out.as_ref());
        let mut reader = papokin_nbt::deserializer::NbtReadHelperJava::new(
            papokin_nbt::deserializer::NbtStreamReader(&mut cursor),
        );
        let nbt = papokin_nbt::Nbt::read(&mut reader).expect("重新解析输出");
        let root = &nbt.root_tag;

        assert_eq!(root.get_long("LastUpdate"), Some(123_456));
        assert_eq!(root.get_long("InhabitedTime"), Some(77));
        assert!(root.get_compound("structures").is_some_and(|structures| {
            structures
                .get_compound("starts")
                .is_some_and(|starts| starts.get_compound("minecraft:fortress").is_some())
        }));
        assert!(
            root.get_compound("blending_data")
                .is_some_and(|blending| blending.get_int("min_section_x") == Some(5))
        );
        assert!(root.get_compound("Heightmaps").is_some_and(|heightmaps| {
            heightmaps
                .get_long_array("OCEAN_FLOOR")
                .is_some_and(|arr| arr.iter().all(|&value| value == 7))
        }));
        // 只要状态不变，旧的状态写法就会延续下去。
        assert_eq!(root.get_string("Status"), Some("minecraft:noise"));
        // PDC 保存在 `BukkitValues` 下，而不是 `PumpkinCustomData`。
        assert!(
            root.get_compound("BukkitValues")
                .is_some_and(|bukkit| bukkit.get_string("papo-key") == Some("papo-value"))
        );
        assert!(!root.has("PumpkinCustomData"));

        // 第二次往返必须保持稳定。
        let chunk = ChunkData::internal_from_bytes(&out, Vector2::new(5, -3)).expect("重新解析");
        let out2 = chunk.internal_to_bytes();
        let mut cursor = std::io::Cursor::new(out2.as_ref());
        let mut reader = papokin_nbt::deserializer::NbtReadHelperJava::new(
            papokin_nbt::deserializer::NbtStreamReader(&mut cursor),
        );
        let nbt2 = papokin_nbt::Nbt::read(&mut reader).expect("再次重新解析");
        assert_eq!(
            nbt2.root_tag.child_tags.len(),
            nbt.root_tag.child_tags.len()
        );
        assert_eq!(nbt2.get_long("LastUpdate"), Some(123_456));
        assert_eq!(nbt2.get_string("Status"), Some("minecraft:noise"));
    }
}
