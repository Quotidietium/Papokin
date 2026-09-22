use papokin_data::packet::clientbound::play::SPAWN_PAINTING;
use papokin_data::painting_variant_id_remap::remap_motive_id_for_version;
use papokin_macros::java_packet;
use papokin_util::math::position::BlockPos;
use papokin_util::version::JavaMinecraftVersion;
use uuid::Uuid;

use crate::{
    ClientPacket, ServerPacket, VarInt,
    ser::{NetworkReadExt, NetworkWriteExt, ReadingError, WritingError},
};

/// 在世界中生成画实体，适用于 <= 1.18.2 的版本。
/// 在 1.19+ 中，画通过统一的 `CSpawnEntity` 数据包生成。
#[derive(Clone, Debug, PartialEq, Eq)]
#[java_packet(SPAWN_PAINTING)]
pub struct CSpawnPainting {
    pub entity_id: VarInt,
    pub uuid: Uuid,
    pub title: String,
    pub variant: VarInt,
    pub location: BlockPos,
    pub direction: u8,
}

impl CSpawnPainting {
    #[must_use]
    pub const fn new(
        entity_id: VarInt,
        uuid: Uuid,
        title: String,
        variant: VarInt,
        location: BlockPos,
        direction: u8,
    ) -> Self {
        Self {
            entity_id,
            uuid,
            title,
            variant,
            location,
            direction,
        }
    }
}

impl ClientPacket for CSpawnPainting {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        write.write_var_int(&self.entity_id)?;
        if *version >= JavaMinecraftVersion::V_1_9 {
            write.write_uuid(&self.uuid)?;
        }
        if *version >= JavaMinecraftVersion::V_1_13 {
            let remapped_variant = remap_motive_id_for_version(self.variant.0 as u32, *version);
            write.write_var_int(&VarInt(remapped_variant as i32))?;
        } else {
            write.write_string_bounded(&self.title, 13)?;
        }

        if *version >= JavaMinecraftVersion::V_1_8 {
            write.write_block_pos(&self.location, version)?;
            write.write_u8(self.direction)?;
        } else {
            write.write_i32_be(self.location.0.x)?;
            write.write_i32_be(self.location.0.y)?;
            write.write_i32_be(self.location.0.z)?;
            write.write_i32_be(self.direction as i32)?;
        }

        Ok(())
    }
}

impl<'a> ServerPacket<'a> for CSpawnPainting {
    fn read(bytebuf: &mut &'a [u8], version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let entity_id = bytebuf.get_var_int()?;
        let uuid = if *version >= JavaMinecraftVersion::V_1_9 {
            bytebuf.get_uuid()?
        } else {
            Uuid::nil()
        };

        let (title, variant) = if *version >= JavaMinecraftVersion::V_1_13 {
            let variant = bytebuf.get_var_int()?;
            (String::new(), variant)
        } else {
            let title = bytebuf.get_str()?.to_string();
            (title, VarInt(0))
        };

        let (location, direction) = if *version >= JavaMinecraftVersion::V_1_8 {
            let loc = bytebuf.get_block_pos(version)?;
            let dir = bytebuf.get_u8()?;
            (loc, dir)
        } else {
            let x = bytebuf.get_i32_be()?;
            let y = bytebuf.get_i32_be()?;
            let z = bytebuf.get_i32_be()?;
            let dir = bytebuf.get_i32_be()? as u8;
            (BlockPos::new(x, y, z), dir)
        };

        Ok(Self {
            entity_id,
            uuid,
            title,
            variant,
            location,
            direction,
        })
    }
}
