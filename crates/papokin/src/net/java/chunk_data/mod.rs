pub mod light;
pub mod util;
pub mod v1_18;
pub mod v1_7;
pub mod v1_8;
pub mod v1_9;

pub use light::ChunkLightExt;

use papokin_data::packet::clientbound::play::LEVEL_CHUNK_WITH_LIGHT;
use papokin_protocol::ClientPacket;
use papokin_protocol::packet::MultiVersionJavaPacket;
use papokin_protocol::ser::WritingError;
use papokin_util::version::JavaMinecraftVersion;
use papokin_world::chunk::ChunkData;
use std::io::Write;

/// 由服务器发送，用于向客户端提供区块的完整数据。
///
/// 这包括高度图、实际的方块和生物群系数据（按区块段组织），
/// 方块实体（如告示牌或箱子），以及两者的光照等级信息
/// 天空光和方块光。
pub struct CChunkData<'a>(pub &'a ChunkData);

impl MultiVersionJavaPacket for CChunkData<'_> {
    fn to_id(version: JavaMinecraftVersion) -> i32 {
        LEVEL_CHUNK_WITH_LIGHT.to_id(version)
    }
}

impl<'a> CChunkData<'a> {
    #[must_use]
    pub const fn new(chunk: &'a ChunkData) -> Self {
        Self(chunk)
    }
}

impl ClientPacket for CChunkData<'_> {
    fn write_packet_data(
        &self,
        write: impl Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        if version >= &JavaMinecraftVersion::V_1_18 {
            v1_18::write_chunk_data(self.0, write, version)
        } else if version >= &JavaMinecraftVersion::V_1_9 {
            v1_9::write_chunk_data(self.0, write, version)
        } else if version == &JavaMinecraftVersion::V_1_8 {
            v1_8::write_chunk_data(self.0, write, version)
        } else if version == &JavaMinecraftVersion::V_1_7_2
            || version == &JavaMinecraftVersion::V_1_7_6
        {
            v1_7::write_chunk_data(self.0, write, version)
        } else {
            v1_18::write_chunk_data(self.0, write, version)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use papokin_data::block_state_remap::BLOCK_STATE_REMAP_V_26_3_TO_V_1_21_11;
    use papokin_world::chunk::ChunkData;

    #[test]
    fn chunk_data_all_versions() {
        let chunk = ChunkData::empty(0, 0);
        let packet = CChunkData(&chunk);

        let versions = [
            JavaMinecraftVersion::V_1_7_2,
            JavaMinecraftVersion::V_1_7_6,
            JavaMinecraftVersion::V_1_8,
            JavaMinecraftVersion::V_1_9,
            JavaMinecraftVersion::V_1_12_2,
            JavaMinecraftVersion::V_1_13_2,
            JavaMinecraftVersion::V_1_14_4,
            JavaMinecraftVersion::V_1_15_2,
            JavaMinecraftVersion::V_1_16_1,
            JavaMinecraftVersion::V_1_16_4,
            JavaMinecraftVersion::V_1_17_1,
            JavaMinecraftVersion::V_1_18_2,
            JavaMinecraftVersion::V_1_19_4,
            JavaMinecraftVersion::V_1_20_2,
            JavaMinecraftVersion::V_1_21_4,
            JavaMinecraftVersion::V_1_21_5,
            JavaMinecraftVersion::V_1_21_11,
            JavaMinecraftVersion::V_26_1,
            JavaMinecraftVersion::V_26_2,
            JavaMinecraftVersion::V_26_3,
        ];

        for version in versions {
            let mut buf = Vec::new();
            let id = CChunkData::to_id(version);
            assert_ne!(id, -1, "Packet ID for version {version:?} must be valid");
            assert!(
                packet.write_packet_data(&mut buf, &version).is_ok(),
                "Failed to serialize chunk data for version {version:?}"
            );
            assert!(
                !buf.is_empty(),
                "Serialized buffer must not be empty for version {version:?}"
            );
        }
    }

    #[test]
    fn populated_chunk_data_all_versions() {
        let chunk = ChunkData::empty(0, 0);
        chunk
            .section
            .set_block_absolute_y(0, 64, 0, papokin_data::Block::STONE.default_state.id);
        chunk
            .section
            .set_block_absolute_y(1, 64, 1, papokin_data::Block::DIRT.default_state.id);

        let mut nbt = papokin_nbt::compound::NbtCompound::new();
        nbt.put_string("id", "minecraft:chest".to_string());
        chunk.pending_block_entities.lock().unwrap().insert(
            papokin_util::math::position::BlockPos(papokin_util::math::vector3::Vector3::new(
                0, 64, 0,
            )),
            nbt,
        );

        let packet = CChunkData(&chunk);

        let versions = [
            JavaMinecraftVersion::V_1_7_2,
            JavaMinecraftVersion::V_1_7_6,
            JavaMinecraftVersion::V_1_8,
            JavaMinecraftVersion::V_1_9,
            JavaMinecraftVersion::V_1_12_2,
            JavaMinecraftVersion::V_1_13_2,
            JavaMinecraftVersion::V_1_14_4,
            JavaMinecraftVersion::V_1_15_2,
            JavaMinecraftVersion::V_1_16_1,
            JavaMinecraftVersion::V_1_16_4,
            JavaMinecraftVersion::V_1_17_1,
            JavaMinecraftVersion::V_1_18_2,
            JavaMinecraftVersion::V_1_19_4,
            JavaMinecraftVersion::V_1_20_2,
            JavaMinecraftVersion::V_1_21_4,
            JavaMinecraftVersion::V_1_21_5,
            JavaMinecraftVersion::V_1_21_11,
            JavaMinecraftVersion::V_26_1,
            JavaMinecraftVersion::V_26_2,
            JavaMinecraftVersion::V_26_3,
        ];

        for version in versions {
            let mut buf = Vec::new();
            let id = CChunkData::to_id(version);
            assert_ne!(id, -1, "Packet ID for version {version:?} must be valid");
            assert!(
                packet.write_packet_data(&mut buf, &version).is_ok(),
                "Failed to serialize populated chunk data for version {version:?}"
            );
            assert!(
                !buf.is_empty(),
                "Serialized buffer must not be empty for version {version:?}"
            );
        }
    }

    /// 从 `pos` 处开始读取一个 `VarInt`（LEB128），并使 `pos` 前进。
    fn read_var_int(buf: &[u8], pos: &mut usize) -> i32 {
        let mut value = 0u32;
        for i in 0..5 {
            let byte = buf[*pos];
            *pos += 1;
            value |= u32::from(byte & 0x7F) << (i * 7);
            if byte & 0x80 == 0 {
                return value as i32;
            }
        }
        panic!("测试缓冲区中的 varint 未终结");
    }

    /// 从 `level_chunk_with_light` 负载中提取区块段缓冲区
    /// 为使用 1.21.5+ 索引高度图格式的版本而编写。
    fn extract_section_buffer(buf: &[u8]) -> &[u8] {
        let mut pos = 8; // 区块 x + z
        let heightmap_count = read_var_int(buf, &mut pos);
        for _ in 0..heightmap_count {
            let _index = read_var_int(buf, &mut pos);
            let len = read_var_int(buf, &mut pos);
            pos += len as usize * 8;
        }
        let sections_len = read_var_int(buf, &mut pos) as usize;
        &buf[pos..pos + sections_len]
    }

    /// 遍历每个区块段并收集方块调色板条目（跳过
    /// 打包数据，其字节绝不能被误当作调色板 id）。
    /// 返回每个区块段的 `(block palettes, biome palettes)`。
    fn collect_block_palettes(
        sections: &[u8],
        num_sections: usize,
        has_liquid_count: bool,
    ) -> (Vec<Vec<i32>>, Vec<Vec<i32>>) {
        let mut pos = 0;
        let mut palettes = Vec::with_capacity(num_sections);
        let mut biome_palettes = Vec::with_capacity(num_sections);
        for _ in 0..num_sections {
            let mut biome_palette = Vec::new();
            pos += 2; // 非空气方块数
            if has_liquid_count {
                pos += 2;
            }

            let bits = usize::from(sections[pos]);
            pos += 1;
            let palette = match bits {
                0 => vec![read_var_int(sections, &mut pos)],
                b if b <= 8 => {
                    let len = read_var_int(sections, &mut pos);
                    (0..len).map(|_| read_var_int(sections, &mut pos)).collect()
                }
                _ => Vec::new(), // 直接调色板：原始 id，无需收集
            };
            if let Some(word_bits) = 64usize.checked_div(bits) {
                pos += 4096usize.div_ceil(word_bits) * 8;
            }

            // 区块段的生物群系部分：跳过调色板与压缩数据。
            let biome_bits = usize::from(sections[pos]);
            pos += 1;
            let biome_words = match biome_bits {
                0 => {
                    biome_palette.push(read_var_int(sections, &mut pos));
                    0
                }
                b => {
                    if b <= 3 {
                        let len = read_var_int(sections, &mut pos);
                        for _ in 0..len {
                            biome_palette.push(read_var_int(sections, &mut pos));
                        }
                    }
                    64usize.div_ceil(64 / b)
                }
            };
            pos += biome_words * 8;

            palettes.push(palette);
            biome_palettes.push(biome_palette);
        }
        assert_eq!(
            pos,
            sections.len(),
            "parser must consume the section buffer exactly"
        );
        (palettes, biome_palettes)
    }

    #[test]
    fn chunk_data_remaps_block_states_for_1_21_11() {
        // 针对 1.21.11 断连（"No value with id ..."）的回归测试：
        // 方块状态 id 来自数据集的原生注册表（26.3），且
        // 必须向下转换到该连接的协议版本。如果
        // `v1_18::write_chunk_data` 中的门控进行过比较的
        // 协议目标而不是再次使用数据集版本，原始的 26.3
        // id 泄漏到网络封包中，导致 1.21.11 客户端断开连接。
        let air_native = papokin_data::Block::AIR.default_state.id.as_u16();
        let air_1_21_11 = BLOCK_STATE_REMAP_V_26_3_TO_V_1_21_11[usize::from(air_native)];

        // 挑选一个在 26.3 与 1.21.11 之间 ID 不同、且
        // 原始/转换后的 id 不得与填充用的空气条目冲突
        // 所有其他区段（section）的调色板。
        let (raw, translated) = BLOCK_STATE_REMAP_V_26_3_TO_V_1_21_11
            .iter()
            .enumerate()
            .map(|(id, &translated)| (id as u16, translated))
            .find(|&(id, translated)| {
                id != air_native
                    && id != air_1_21_11
                    && translated != id
                    && translated != air_1_21_11
            })
            .expect("26.3 与 1.21.11 必须具有不同的状态 ID");

        let chunk = ChunkData::empty(0, 0);
        chunk
            .section
            .set_block_absolute_y(0, 64, 0, papokin_data::BlockStateId::new_or_air(raw));
        let num_sections = chunk.section.block_sections.read().unwrap().len();
        let packet = CChunkData(&chunk);

        let serialize = |version: JavaMinecraftVersion| {
            let mut buf = Vec::new();
            packet.write_packet_data(&mut buf, &version).unwrap();
            buf
        };

        let native_buf = serialize(JavaMinecraftVersion::V_26_3);
        let old_buf = serialize(JavaMinecraftVersion::V_1_21_11);

        let (native_palettes, _) =
            collect_block_palettes(extract_section_buffer(&native_buf), num_sections, true);
        let (old_palettes, _) =
            collect_block_palettes(extract_section_buffer(&old_buf), num_sections, false);

        assert!(
            native_palettes
                .iter()
                .any(|palette| palette.contains(&i32::from(raw))),
            "native (26.3) connections must receive the raw dataset id"
        );
        assert!(
            old_palettes
                .iter()
                .any(|palette| palette.contains(&i32::from(translated))),
            "1.21.11 connections must receive the translated id {translated} (raw {raw})"
        );
        assert!(
            !old_palettes
                .iter()
                .any(|palette| palette.contains(&i32::from(raw))),
            "raw 26.3 id {raw} must never leak into a 1.21.11 palette"
        );
    }

    #[test]
    fn chunk_data_remaps_biomes_for_1_21_11() {
        // 针对同一问题生物群系部分的回归测试：区块生物群系
        // 调色板引用的是服务器的数据集 id 空间，但客户端
        // 用其在配置阶段收到的注册表数据来解析它们
        // 时间。65 个生物群系中有 57 个在 1.21.11 里位于不同索引，而且
        // 两个仅存在于 26.x 的生物群系在那里根本不存在——原始 id 顶多
        // 轻则是悄悄出错的生物群系，重则是超出范围的调色板条目。
        use papokin_data::biome::Biome;
        use papokin_data::sync_id_remap::BIOME_SYNC_REMAP_V_26_3_TO_V_1_21_11;

        let forest = u16::from(Biome::FOREST.id);
        let expected = BIOME_SYNC_REMAP_V_26_3_TO_V_1_21_11[usize::from(forest)];
        assert_ne!(forest, expected, "forest must shift between versions");

        let chunk = ChunkData::empty(0, 0);
        let min_y = chunk.section.min_y;
        chunk.section.set_relative_biome(
            0,
            usize::try_from((64 - min_y) / 4).unwrap(),
            0,
            Biome::FOREST.id,
        );
        let num_sections = chunk.section.block_sections.read().unwrap().len();
        let packet = CChunkData(&chunk);

        let serialize = |version: JavaMinecraftVersion| {
            let mut buf = Vec::new();
            packet.write_packet_data(&mut buf, &version).unwrap();
            buf
        };

        let (_, native_biomes) = collect_block_palettes(
            extract_section_buffer(&serialize(JavaMinecraftVersion::V_26_3)),
            num_sections,
            true,
        );
        let (_, old_biomes) = collect_block_palettes(
            extract_section_buffer(&serialize(JavaMinecraftVersion::V_1_21_11)),
            num_sections,
            false,
        );

        assert!(
            native_biomes
                .iter()
                .any(|palette| palette.contains(&i32::from(forest))),
            "native (26.3) connections must receive the raw dataset biome id"
        );
        assert!(
            old_biomes
                .iter()
                .any(|palette| palette.contains(&i32::from(expected))),
            "1.21.11 connections must receive the synced id {expected} (raw {forest})"
        );
        assert!(
            !old_biomes
                .iter()
                .any(|palette| palette.contains(&i32::from(forest))),
            "raw dataset biome id {forest} must never leak into a 1.21.11 palette"
        );
    }
}
