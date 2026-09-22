use std::io::Write;

use papokin_data::packet::clientbound::play::USE_BED;
use papokin_macros::java_packet;
use papokin_util::{math::position::BlockPos, version::JavaMinecraftVersion};

use crate::{
    ClientPacket, ServerPacket, VarInt,
    ser::{NetworkReadExt, NetworkWriteExt, ReadingError, WritingError},
};

/// 由服务器发送，用于告知客户端某玩家正在床上。
///
/// 此数据包用于 1.13.2 及更早版本。在 1.14+ 中，它被移除
/// 并通过实体元数据 / 姿势来指示睡眠状态。
#[java_packet(USE_BED)]
pub struct CUseBed {
    /// 正在睡觉的玩家的实体 ID。
    pub entity_id: VarInt,
    /// 床头的方块位置。
    pub location: BlockPos,
}

impl CUseBed {
    #[must_use]
    pub const fn new(entity_id: VarInt, location: BlockPos) -> Self {
        Self {
            entity_id,
            location,
        }
    }
}

impl ClientPacket for CUseBed {
    fn write_packet_data(
        &self,
        mut write: impl Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        if *version >= JavaMinecraftVersion::V_1_8 {
            write.write_var_int(&self.entity_id)?;
            write.write_block_pos(&self.location, version)?;
        } else {
            write.write_i32_be(self.entity_id.0)?;
            write.write_i32_be(self.location.0.x)?;
            write.write_u8(self.location.0.y as u8)?;
            write.write_i32_be(self.location.0.z)?;
        }

        Ok(())
    }
}

impl<'a> ServerPacket<'a> for CUseBed {
    fn read(read: &mut &'a [u8], version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        if *version >= JavaMinecraftVersion::V_1_8 {
            let entity_id = read.get_var_int()?;
            let location = read.get_block_pos(version)?;
            Ok(Self {
                entity_id,
                location,
            })
        } else {
            let entity_id = VarInt(read.get_i32_be()?);
            let x = read.get_i32_be()?;
            let y = i32::from(read.get_u8()?);
            let z = read.get_i32_be()?;
            let location = BlockPos::new(x, y, z);
            Ok(Self {
                entity_id,
                location,
            })
        }
    }
}
