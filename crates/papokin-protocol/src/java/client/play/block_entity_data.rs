use std::io::{Cursor, Read, Write};

use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use papokin_data::block_entity_type_id_remap::remap_block_entity_type_id_for_version;
use papokin_data::packet::clientbound::play::BLOCK_ENTITY_DATA;
use papokin_macros::java_packet;
use papokin_nbt::{COMPOUND_ID, Nbt, deserializer::NbtReadHelperJava};
use papokin_util::text::sign::remap_block_entity_sign_nbt;
use papokin_util::{math::position::BlockPos, version::JavaMinecraftVersion};

use crate::{
    ClientPacket, ServerPacket, VarInt,
    ser::{NetworkReadExt, NetworkWriteExt, ReadingError, WritingError},
};

/// 更新方块实体的 NBT 数据（例如告示牌、箱子或旗帜）。
///
/// 当方块实体的状态发生变化时，服务器会发送此数据包
/// (如告示牌上的文字)，或在方块实体被加载到客户端视图中时。
#[java_packet(BLOCK_ENTITY_DATA)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CBlockEntityData {
    /// 方块实体的世界坐标。
    pub location: BlockPos,
    /// 正在更新的方块实体类型（例如刷怪笼、命令方块）。
    pub r#type: VarInt,
    /// 包含方块专有数据的原始 NBT 负载。
    pub nbt_data: Box<[u8]>,
}

impl CBlockEntityData {
    #[must_use]
    pub const fn new(location: BlockPos, r#type: VarInt, nbt_data: Box<[u8]>) -> Self {
        Self {
            location,
            r#type,
            nbt_data,
        }
    }
}

fn looks_like_sign_nbt(nbt_data: &[u8]) -> bool {
    nbt_data.windows(10).any(|w| w == b"front_text")
        || nbt_data.windows(9).any(|w| w == b"back_text")
}

fn remap_sign_nbt_payload(nbt_data: &[u8], version: JavaMinecraftVersion) -> Option<Vec<u8>> {
    if version >= JavaMinecraftVersion::V_1_21_5
        || nbt_data.len() < 2
        || nbt_data[0] != COMPOUND_ID
        || !looks_like_sign_nbt(nbt_data)
    {
        return None;
    }

    let mut cursor = Cursor::new(nbt_data);
    let mut reader = NbtReadHelperJava::new(&mut cursor);
    let mut compound = Nbt::read_unnamed(&mut reader).ok()?.root_tag;

    remap_block_entity_sign_nbt(&mut compound, version)
        .then(|| Nbt::from(compound).write_unnamed().to_vec())
}

pub fn write_nbt_payload(
    mut write: impl Write,
    nbt_data: &[u8],
    version: &JavaMinecraftVersion,
) -> Result<(), WritingError> {
    let remapped = remap_sign_nbt_payload(nbt_data, *version);
    let nbt_data = remapped.as_deref().unwrap_or(nbt_data);

    if *version >= JavaMinecraftVersion::V_1_8 {
        if nbt_data.is_empty() || nbt_data == [0] {
            write.write_u8(0)?;
        } else if *version < JavaMinecraftVersion::V_1_20_2 {
            // 在 1.8..1.20.1 中，根复合标签已命名。
            // 如果 nbt_data 是未命名的复合标签（0x0A 后跟内容），则插入空名称（0x00, 0x00）。
            if nbt_data.len() >= 3 && nbt_data[0] == 0x0A && nbt_data[1] == 0 && nbt_data[2] == 0 {
                write.write_all(nbt_data).map_err(WritingError::IoError)?;
            } else if nbt_data[0] == 0x0A {
                write.write_u8(0x0A)?;
                write.write_u16_be(0)?;
                write
                    .write_all(&nbt_data[1..])
                    .map_err(WritingError::IoError)?;
            } else {
                write.write_all(nbt_data).map_err(WritingError::IoError)?;
            }
        } else {
            // 在 1.20.2+ 中，根复合标签未命名。
            write.write_all(nbt_data).map_err(WritingError::IoError)?;
        }
    } else {
        // <= 1.7.6
        if nbt_data.is_empty() || nbt_data == [0] {
            write.write_i16_be(-1)?;
        } else {
            let mut named_bytes = Vec::with_capacity(nbt_data.len() + 2);
            if nbt_data.len() >= 3 && nbt_data[0] == 0x0A && nbt_data[1] == 0 && nbt_data[2] == 0 {
                named_bytes.extend_from_slice(nbt_data);
            } else if nbt_data[0] == 0x0A {
                named_bytes.push(0x0A);
                named_bytes.extend_from_slice(&[0x00, 0x00]);
                named_bytes.extend_from_slice(&nbt_data[1..]);
            } else {
                named_bytes.extend_from_slice(nbt_data);
            }

            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder
                .write_all(&named_bytes)
                .map_err(WritingError::IoError)?;
            let compressed = encoder.finish().map_err(WritingError::IoError)?;

            write.write_i16_be(compressed.len() as i16)?;
            write
                .write_all(&compressed)
                .map_err(WritingError::IoError)?;
        }
    }
    Ok(())
}

pub fn read_nbt_payload(
    bytebuf: &mut &[u8],
    version: &JavaMinecraftVersion,
) -> Result<Box<[u8]>, ReadingError> {
    if *version >= JavaMinecraftVersion::V_1_8 {
        if bytebuf.is_empty() || bytebuf[0] == 0 {
            if !bytebuf.is_empty() {
                let _ = bytebuf.get_u8()?;
            }
            Ok(Box::new([]))
        } else if *version < JavaMinecraftVersion::V_1_20_2 {
            let all = bytebuf.to_vec();
            *bytebuf = &[];
            if all.len() >= 3 && all[0] == 0x0A && all[1] == 0 && all[2] == 0 {
                let mut unnamed = Vec::with_capacity(all.len() - 2);
                unnamed.push(0x0A);
                unnamed.extend_from_slice(&all[3..]);
                Ok(unnamed.into_boxed_slice())
            } else {
                Ok(all.into_boxed_slice())
            }
        } else {
            let all = bytebuf.to_vec().into_boxed_slice();
            *bytebuf = &[];
            Ok(all)
        }
    } else {
        // <= 1.7.6
        let length = bytebuf.get_i16_be()?;
        if length <= 0 {
            Ok(Box::new([]))
        } else {
            if bytebuf.len() < length as usize {
                return Err(ReadingError::Incomplete(
                    "Not enough bytes for compressed NBT".into(),
                ));
            }
            let compressed = &bytebuf[..length as usize];
            *bytebuf = &bytebuf[length as usize..];
            let mut decoder = GzDecoder::new(compressed);
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| ReadingError::Message(e.to_string()))?;
            if decompressed.len() >= 3
                && decompressed[0] == 0x0A
                && decompressed[1] == 0
                && decompressed[2] == 0
            {
                let mut unnamed = Vec::with_capacity(decompressed.len() - 2);
                unnamed.push(0x0A);
                unnamed.extend_from_slice(&decompressed[3..]);
                Ok(unnamed.into_boxed_slice())
            } else {
                Ok(decompressed.into_boxed_slice())
            }
        }
    }
}

impl ClientPacket for CBlockEntityData {
    fn write_packet_data(
        &self,
        mut write: impl Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        write.write_block_pos(&self.location, version)?;

        let remapped_type = remap_block_entity_type_id_for_version(self.r#type.0 as u32, *version);
        if *version >= JavaMinecraftVersion::V_1_18 {
            write.write_var_int(&VarInt(remapped_type as i32))?;
        } else {
            write.write_u8(remapped_type as u8)?;
        }

        write_nbt_payload(&mut write, &self.nbt_data, version)
    }
}

impl<'a> ServerPacket<'a> for CBlockEntityData {
    fn read(bytebuf: &mut &'a [u8], version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let location = bytebuf.get_block_pos(version)?;
        let r#type = if *version >= JavaMinecraftVersion::V_1_18 {
            bytebuf.get_var_int()?
        } else {
            VarInt(i32::from(bytebuf.get_u8()?))
        };
        let nbt_data = read_nbt_payload(bytebuf, version)?;
        Ok(Self {
            location,
            r#type,
            nbt_data,
        })
    }
}
