use crate::{
    ServerPacket,
    ser::{NetworkReadExt, ReadingError},
};
use papokin_data::packet::serverbound::play::{PUNCH, SWING};
use papokin_util::version::JavaMinecraftVersion;

use crate::VarInt;

pub struct SSwingArm {
    pub hand: VarInt,
}

/// 26.3 用 punch（挥击）取代了 swing（摆手）数据包，新数据包不再告知使用的是哪只手。
impl crate::packet::MultiVersionJavaPacket for SSwingArm {
    fn to_id(version: JavaMinecraftVersion) -> i32 {
        if version >= JavaMinecraftVersion::V_26_3 {
            PUNCH.to_id(version)
        } else {
            SWING.to_id(version)
        }
    }
}

impl<'a> ServerPacket<'a> for SSwingArm {
    fn read(bytebuf: &mut &'a [u8], version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let hand = if version >= &JavaMinecraftVersion::V_26_3 {
            VarInt(0)
        } else if version >= &JavaMinecraftVersion::V_1_9 {
            bytebuf.get_var_int()?
        } else if version >= &JavaMinecraftVersion::V_1_8 {
            VarInt(0)
        } else {
            let _entity_id = bytebuf.get_i32_be()?;
            let _animation = bytebuf.get_u8()?;
            VarInt(0)
        };
        Ok(Self { hand })
    }
}

impl crate::ClientPacket for SSwingArm {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        use crate::ser::NetworkWriteExt;
        if version >= &JavaMinecraftVersion::V_26_3 {
            // 挥拳数据包没有任何字段
        } else if version >= &JavaMinecraftVersion::V_1_9 {
            write.write_var_int(&self.hand)?;
        } else if version <= &JavaMinecraftVersion::V_1_7_6 {
            write.write_i32_be(0)?;
            write.write_u8(1)?;
        }
        Ok(())
    }
}
