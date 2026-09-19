pub mod light;
pub mod util;
pub mod v1_18;
pub mod v1_7;
pub mod v1_8;
pub mod v1_9;

pub use light::ChunkLightExt;

use pumpkin_data::packet::clientbound::play::LEVEL_CHUNK_WITH_LIGHT;
use pumpkin_protocol::ClientPacket;
use pumpkin_protocol::packet::MultiVersionJavaPacket;
use pumpkin_protocol::ser::WritingError;
use pumpkin_util::version::JavaMinecraftVersion;
use pumpkin_world::chunk::ChunkData;
use std::io::Write;

/// Sent by the server to provide the client with the full data for a chunk.
///
/// This includes heightmaps, the actual block and biome data (organized into sections),
/// block entities (like signs or chests), and the light level information for both
/// sky and block light.
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
    use pumpkin_data::block_state_remap::BLOCK_STATE_REMAP_V_26_3_TO_V_1_21_11;
    use pumpkin_world::chunk::ChunkData;

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
            .set_block_absolute_y(0, 64, 0, pumpkin_data::Block::STONE.default_state.id);
        chunk
            .section
            .set_block_absolute_y(1, 64, 1, pumpkin_data::Block::DIRT.default_state.id);

        let mut nbt = pumpkin_nbt::compound::NbtCompound::new();
        nbt.put_string("id", "minecraft:chest".to_string());
        chunk.pending_block_entities.lock().unwrap().insert(
            pumpkin_util::math::position::BlockPos(pumpkin_util::math::vector3::Vector3::new(
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

    /// Reads one VarInt (LEB128) starting at `pos`, advancing it.
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
        panic!("unterminated varint in test buffer");
    }

    /// Extracts the section buffer from a `level_chunk_with_light` payload
    /// written for a version using the 1.21.5+ indexed heightmap format.
    fn extract_section_buffer(buf: &[u8]) -> &[u8] {
        let mut pos = 8; // chunk x + z
        let heightmap_count = read_var_int(buf, &mut pos);
        for _ in 0..heightmap_count {
            let _index = read_var_int(buf, &mut pos);
            let len = read_var_int(buf, &mut pos);
            pos += len as usize * 8;
        }
        let sections_len = read_var_int(buf, &mut pos) as usize;
        &buf[pos..pos + sections_len]
    }

    /// Walks every section and collects the block palette entries (skipping
    /// the packed data, whose bytes must not be mistaken for palette ids).
    /// Returns `(block palettes, biome palettes)` per section.
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
            pos += 2; // non-air count
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
                _ => Vec::new(), // direct palette: raw ids, nothing to collect
            };
            if bits > 0 {
                pos += 4096usize.div_ceil(64 / bits) * 8;
            }

            // Biome half of the section: skip palette and packed data.
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
        // Regression test for the 1.21.11 disconnect ("No value with id ..."):
        // block-state ids come from the dataset's native registry (26.3) and
        // must be translated down to the connection's protocol version. If
        // the gate in `v1_18::write_chunk_data` ever compares against the
        // protocol target instead of the dataset version again, the raw 26.3
        // id leaks onto the wire and 1.21.11 clients disconnect.
        let air_native = pumpkin_data::Block::AIR.default_state.id.as_u16();
        let air_1_21_11 = BLOCK_STATE_REMAP_V_26_3_TO_V_1_21_11[usize::from(air_native)];

        // Pick a state whose id differs between 26.3 and 1.21.11, and whose
        // raw/translated ids cannot collide with the air entries that fill
        // every other section's palette.
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
            .expect("26.3 and 1.21.11 must have differing state ids");

        let chunk = ChunkData::empty(0, 0);
        chunk
            .section
            .set_block_absolute_y(0, 64, 0, pumpkin_data::BlockStateId::new_or_air(raw));
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
        // Regression test for the biome half of the same problem: chunk biome
        // palettes reference the server's dataset id space, but the client
        // resolves them against the registry data it received at configuration
        // time. 57 of 65 biomes sit at different indices in 1.21.11, and the
        // two 26.x-only biomes do not exist there at all — a raw id is at best
        // a silently wrong biome and at worst an out-of-range palette entry.
        use pumpkin_data::biome::Biome;
        use pumpkin_data::sync_id_remap::BIOME_SYNC_REMAP_V_26_3_TO_V_1_21_11;

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
