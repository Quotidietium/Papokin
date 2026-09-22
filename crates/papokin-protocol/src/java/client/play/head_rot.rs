use papokin_data::packet::clientbound::play::ROTATE_HEAD;
use papokin_macros::java_packet;

use crate::{
    ClientPacket, ServerPacket, VarInt,
    ser::{NetworkReadExt, NetworkWriteExt, ReadingError},
};
use papokin_util::version::JavaMinecraftVersion;

/// 将实体的头部旋转到指定偏航角。
///
/// 在 Minecraft 中，实体的“身体偏航角”与“头部偏航角”是分开的。
/// 标准移动数据包更新的是身体，而此数据包
/// 让实体（如玩家或生物）看向某个
/// 特定方向，而不必转动整个身体。
#[java_packet(ROTATE_HEAD)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CHeadRot {
    /// 头部正在旋转的实体 ID。
    pub entity_id: VarInt,
    /// 新的头部偏航角，以整圈的 1/256 为步长（0-255）。
    pub head_yaw: u8,
}

impl CHeadRot {
    #[must_use]
    pub const fn new(entity_id: VarInt, head_yaw: u8) -> Self {
        Self {
            entity_id,
            head_yaw,
        }
    }
}

impl ClientPacket for CHeadRot {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        if *version <= JavaMinecraftVersion::V_1_7_6 {
            write.write_i32_be(self.entity_id.0)?;
        } else {
            write.write_var_int(&self.entity_id)?;
        }
        write.write_u8(self.head_yaw)?;
        Ok(())
    }
}

impl<'a> ServerPacket<'a> for CHeadRot {
    fn read(bytebuf: &mut &'a [u8], version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let entity_id = if *version <= JavaMinecraftVersion::V_1_7_6 {
            VarInt(bytebuf.get_i32_be()?)
        } else {
            bytebuf.get_var_int()?
        };
        let head_yaw = bytebuf.get_u8()?;
        Ok(Self {
            entity_id,
            head_yaw,
        })
    }
}
