use super::util::write_compound_nbt;
use papokin_data::NATIVE_DATA_VERSION;
use papokin_data::block_state_remap::remap_block_state_for_version;
use papokin_data::sync_id_remap::remap_biome_id_for_version;
use papokin_protocol::codec::bit_set::BitSet;
use papokin_protocol::codec::var_int::VarInt;
use papokin_protocol::ser::NetworkWriteExt;
use papokin_protocol::ser::WritingError;
use papokin_util::math::position::get_local_cord;
use papokin_util::version::JavaMinecraftVersion;
use papokin_world::chunk::ChunkData;
use papokin_world::chunk::format::LightContainer;
use papokin_world::chunk::palette::NetworkPalette;
use std::io::Write;

/// 序列化 Minecraft 1.18+ 至 26.2+ 的区块数据。
#[expect(clippy::too_many_lines)]
pub fn write_chunk_data(
    chunk: &ChunkData,
    mut write: impl Write,
    version: &JavaMinecraftVersion,
) -> Result<(), WritingError> {
    write.write_i32_be(chunk.x)?;
    write.write_i32_be(chunk.z)?;

    let heightmaps = chunk
        .heightmap
        .lock()
        .map_err(|_| WritingError::Message("heightmap lock poisoned".into()))?;
    if version >= &JavaMinecraftVersion::V_1_21_5 {
        write.write_var_int(&VarInt(3))?; // 地图大小

        let mut write_heightmap = |index: i32, data: &[i64]| -> Result<(), WritingError> {
            write.write_var_int(&VarInt(index))?;
            write.write_var_int(&VarInt(data.len() as i32))?;
            for val in data {
                write.write_i64_be(*val)?;
            }
            Ok(())
        };

        write_heightmap(1, heightmaps.world_surface.as_deref().unwrap_or(&[0; 37]))?;
        write_heightmap(4, heightmaps.motion_blocking.as_deref().unwrap_or(&[0; 37]))?;
        write_heightmap(
            5,
            heightmaps
                .motion_blocking_no_leaves
                .as_deref()
                .unwrap_or(&[0; 37]),
        )?;
    } else {
        let mut comp = papokin_nbt::compound::NbtCompound::new();
        if let Some(ref ws) = heightmaps.world_surface {
            comp.put(
                "WORLD_SURFACE",
                papokin_nbt::tag::NbtTag::LongArray(ws.to_vec()),
            );
        }
        if let Some(ref mb) = heightmaps.motion_blocking {
            comp.put(
                "MOTION_BLOCKING",
                papokin_nbt::tag::NbtTag::LongArray(mb.to_vec()),
            );
        }
        if let Some(ref mbnl) = heightmaps.motion_blocking_no_leaves {
            comp.put(
                "MOTION_BLOCKING_NO_LEAVES",
                papokin_nbt::tag::NbtTag::LongArray(mbnl.to_vec()),
            );
        }
        write_compound_nbt(&mut write, &comp, *version)?;
    }
    drop(heightmaps);

    {
        let mut blocks_and_biomes_buf = Vec::new();
        let block_sections = chunk
            .section
            .block_sections
            .read()
            .map_err(|_| WritingError::Message("block_sections read lock poisoned".into()))?;
        let biome_sections = chunk
            .section
            .biome_sections
            .read()
            .map_err(|_| WritingError::Message("biome_sections read lock poisoned".into()))?;

        let mut zero_bytes_count = 0;

        for (block_palette, biome_palette) in block_sections.iter().zip(biome_sections.iter()) {
            let non_empty_block_count = block_palette.non_air_block_count() as i16;
            blocks_and_biomes_buf.write_i16_be(non_empty_block_count)?;
            if version >= &JavaMinecraftVersion::V_26_1 {
                // 26.1 新增，流体计数
                let liquid_count = block_palette.liquid_block_count() as i16;
                blocks_and_biomes_buf.write_i16_be(liquid_count)?;
            }

            let mut block_network = block_palette.convert_network();
            // 注册表 ID 仅在连接运行于
            // 数据集的原生版本；任何更旧的版本（例如在
            // 26.3 数据集）必须向下换算。
            if version < &NATIVE_DATA_VERSION {
                match &mut block_network.palette {
                    NetworkPalette::Single(registry_id) => {
                        *registry_id = remap_block_state_for_version(*registry_id, *version);
                    }
                    NetworkPalette::Indirect(palette) => {
                        for registry_id in palette.iter_mut() {
                            *registry_id = remap_block_state_for_version(*registry_id, *version);
                        }
                    }
                    NetworkPalette::Direct => {
                        let bits_per_entry = usize::from(block_network.bits_per_entry);
                        let values_per_i64 = 64 / bits_per_entry;
                        let id_mask = (1u64 << bits_per_entry) - 1;

                        for packed_word in &mut block_network.packed_data {
                            let mut remapped_word = 0u64;
                            let packed_word_u64 = *packed_word as u64;
                            for index in 0..values_per_i64 {
                                let shift = index * bits_per_entry;
                                let state_id = ((packed_word_u64 >> shift) & id_mask) as u16;
                                let remapped_id = remap_block_state_for_version(state_id, *version);
                                remapped_word |= u64::from(remapped_id) << shift;
                            }
                            *packed_word = remapped_word as i64;
                        }
                    }
                }
            }
            blocks_and_biomes_buf.write_u8(block_network.bits_per_entry)?;

            match block_network.palette {
                NetworkPalette::Single(registry_id) => {
                    blocks_and_biomes_buf.write_var_int(&registry_id.into())?;
                }
                NetworkPalette::Indirect(palette) => {
                    blocks_and_biomes_buf.write_var_int(&palette.len().try_into().map_err(
                        |_| {
                            WritingError::Message(format!(
                                "{} is not representable as a VarInt!",
                                palette.len()
                            ))
                        },
                    )?)?;
                    for registry_id in palette {
                        blocks_and_biomes_buf.write_var_int(&registry_id.into())?;
                    }
                }
                NetworkPalette::Direct => {}
            }

            if version <= &JavaMinecraftVersion::V_1_21_4 {
                blocks_and_biomes_buf.write_list(&block_network.packed_data, |buf, &packed| {
                    buf.write_i64_be(packed)
                })?;
            } else {
                for packed in &block_network.packed_data {
                    blocks_and_biomes_buf.write_i64_be(*packed)?;
                }
            }

            let mut biome_network = biome_palette.convert_network();
            // 生物群系是同步注册表：客户端解析调色板 ID
            // 与该服务器在配置阶段发送给它的注册表数据不一致
            // 时间，其 id 空间会偏离数据集原本的版本
            // 添加条目（见 `sync_id_remap`）。
            if version < &NATIVE_DATA_VERSION {
                match &mut biome_network.palette {
                    NetworkPalette::Single(registry_id) => {
                        *registry_id = u8::try_from(remap_biome_id_for_version(
                            u16::from(*registry_id),
                            *version,
                        ))
                        .unwrap_or(0);
                    }
                    NetworkPalette::Indirect(palette) => {
                        for registry_id in palette.iter_mut() {
                            *registry_id = u8::try_from(remap_biome_id_for_version(
                                u16::from(*registry_id),
                                *version,
                            ))
                            .unwrap_or(0);
                        }
                    }
                    NetworkPalette::Direct => {
                        let bits_per_entry = usize::from(biome_network.bits_per_entry);
                        let values_per_i64 = 64 / bits_per_entry;
                        let id_mask = (1u64 << bits_per_entry) - 1;

                        for packed_word in &mut biome_network.packed_data {
                            let mut remapped_word = 0u64;
                            let packed_word_u64 = *packed_word as u64;
                            for index in 0..values_per_i64 {
                                let shift = index * bits_per_entry;
                                let biome_id = ((packed_word_u64 >> shift) & id_mask) as u8;
                                let remapped_id =
                                    remap_biome_id_for_version(u16::from(biome_id), *version);
                                remapped_word |= u64::from(remapped_id as u8) << shift;
                            }
                            *packed_word = remapped_word as i64;
                        }
                    }
                }
            }
            blocks_and_biomes_buf.write_u8(biome_network.bits_per_entry)?;

            match biome_network.palette {
                NetworkPalette::Single(registry_id) => {
                    blocks_and_biomes_buf.write_var_int(&registry_id.into())?;
                }
                NetworkPalette::Indirect(palette) => {
                    blocks_and_biomes_buf.write_var_int(&palette.len().try_into().map_err(
                        |_| {
                            WritingError::Message(format!(
                                "{} is not representable as a VarInt!",
                                palette.len()
                            ))
                        },
                    )?)?;
                    for registry_id in palette {
                        blocks_and_biomes_buf.write_var_int(&registry_id.into())?;
                    }
                }
                NetworkPalette::Direct => {}
            }

            if version <= &JavaMinecraftVersion::V_1_21_4 {
                blocks_and_biomes_buf.write_list(&biome_network.packed_data, |buf, &packed| {
                    buf.write_i64_be(packed)
                })?;
            } else {
                for packed in &biome_network.packed_data {
                    blocks_and_biomes_buf.write_i64_be(*packed)?;
                }
            }

            if version == &JavaMinecraftVersion::V_1_21_5 {
                let block_storage_len = block_network.packed_data.len() as i32;
                let biome_storage_len = biome_network.packed_data.len() as i32;
                zero_bytes_count += VarInt(block_storage_len).written_size()
                    + VarInt(biome_storage_len).written_size();
            }
        }

        if version == &JavaMinecraftVersion::V_1_21_5 && zero_bytes_count > 0 {
            blocks_and_biomes_buf.resize(blocks_and_biomes_buf.len() + zero_bytes_count, 0);
        }

        write.write_var_int(&blocks_and_biomes_buf.len().try_into().map_err(|_| {
            WritingError::Message(format!(
                "{} is not representable as a VarInt!",
                blocks_and_biomes_buf.len()
            ))
        })?)?;
        write.write_slice(&blocks_and_biomes_buf)?;
    };

    let block_entities = chunk
        .pending_block_entities
        .lock()
        .map_err(|_| WritingError::Message("block_entities lock poisoned".into()))?;
    write.write_var_int(&VarInt(block_entities.len() as i32))?;
    for (pos, nbt) in block_entities.iter() {
        let local_xz = ((get_local_cord(pos.0.x) & 0xF) << 4) | (get_local_cord(pos.0.z) & 0xF);

        write.write_u8(local_xz as u8)?;
        write.write_i16_be(pos.0.y as i16)?;

        let id = nbt.get_string("id").map_or(0, |id_str| {
            let name = id_str.split(':').next_back().unwrap_or(id_str);
            papokin_data::block_properties::BLOCK_ENTITY_TYPES
                .iter()
                .position(|&n| n == name)
                .unwrap_or(0)
        });
        let remapped_id =
            papokin_data::block_entity_type_id_remap::remap_block_entity_type_id_for_version(
                id as u32, *version,
            );

        write.write_var_int(&VarInt(remapped_id as i32))?;

        let mut client_nbt = nbt.clone();
        client_nbt.child_tags.remove("id");
        client_nbt.child_tags.remove("x");
        client_nbt.child_tags.remove("y");
        client_nbt.child_tags.remove("z");
        client_nbt.child_tags.remove("LootTable");
        client_nbt.child_tags.remove("LootTableSeed");
        client_nbt.child_tags.remove("PumpkinCustomData");
        client_nbt.child_tags.remove("BukkitValues");
        write_compound_nbt(&mut write, &client_nbt, *version)?;
    }

    {
        // 光照掩码包含从 -1（世界下方）到 num_sections（世界上方）的区段
        // 这意味着我们需要在位集中额外计入 2 个 section
        let light_engine = chunk
            .light_engine
            .lock()
            .map_err(|_| WritingError::Message("light_engine lock poisoned".into()))?;
        let num_sections = light_engine.sky_light.len();

        let mut sky_light_empty_mask = 0u64;
        let mut block_light_empty_mask = 0u64;
        let mut sky_light_mask = 0u64;
        let mut block_light_mask = 0u64;

        // 位 0 表示世界下方的区块段（始终为空）
        sky_light_empty_mask |= 1 << 0;
        block_light_empty_mask |= 1 << 0;

        // 位 1..=num_sections 表示实际的世界区块段
        for section_index in 0..num_sections {
            let bit_index = section_index + 1; // 为世界下方区段偏移 1

            if let LightContainer::Full(_) = &light_engine.sky_light[section_index] {
                sky_light_mask |= 1 << bit_index;
            } else {
                sky_light_empty_mask |= 1 << bit_index;
            }

            if let LightContainer::Full(_) = &light_engine.block_light[section_index] {
                block_light_mask |= 1 << bit_index;
            } else {
                block_light_empty_mask |= 1 << bit_index;
            }
        }

        // 位 num_sections+1 表示世界上方的区块段（始终为空）
        sky_light_empty_mask |= 1 << (num_sections + 1);
        block_light_empty_mask |= 1 << (num_sections + 1);

        // 信任边（1.18 - 1.19.4；1.20 移除）
        if version < &JavaMinecraftVersion::V_1_20 {
            write.write_bool(true)?;
        }

        // 写入天空光照掩码
        BitSet(Box::new([sky_light_mask as i64])).encode_with_version(&mut write, version)?;
        // 写入方块光照掩码
        BitSet(Box::new([block_light_mask as i64])).encode_with_version(&mut write, version)?;
        // 写入空的天空光照掩码
        BitSet(Box::new([sky_light_empty_mask as i64])).encode_with_version(&mut write, version)?;
        // 写入空的方块光照掩码
        BitSet(Box::new([block_light_empty_mask as i64]))
            .encode_with_version(&mut write, version)?;

        let light_data_size: VarInt = VarInt(LightContainer::ARRAY_SIZE as i32);

        // 写入天空光照数组
        write.write_var_int(&VarInt(sky_light_mask.count_ones() as i32))?;
        for section_index in 0..num_sections {
            if let LightContainer::Full(data) = &light_engine.sky_light[section_index] {
                write.write_var_int(&light_data_size)?;
                write.write_slice(data.as_ref())?;
            }
        }

        // 写入方块光照数组
        write.write_var_int(&VarInt(block_light_mask.count_ones() as i32))?;
        for section_index in 0..num_sections {
            if let LightContainer::Full(data) = &light_engine.block_light[section_index] {
                write.write_var_int(&light_data_size)?;
                write.write_slice(data.as_ref())?;
            }
        }
    }

    Ok(())
}
