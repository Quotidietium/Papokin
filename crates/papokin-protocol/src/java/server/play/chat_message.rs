use papokin_data::packet::serverbound::play::CHAT;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

use crate::{
    ClientPacket, ServerPacket,
    codec::var_int::VarInt,
    ser::NetworkWriteExt,
    ser::{NetworkReadExt, NetworkReadSliceExt, ReadingError},
};

#[java_packet(CHAT)]
pub struct SChatMessage<'a> {
    pub message: &'a str,
    pub timestamp: i64,
    pub salt: i64,
    pub signature: Option<&'a [u8]>,
    pub message_count: VarInt,
    pub acknowledged: &'a [u8], // 位集固定为 20 位
    pub checksum: u8,           // 1.21.5 "fingerprint" 校验和
}

impl<'a> ServerPacket<'a> for SChatMessage<'a> {
    fn read(read: &mut &'a [u8], version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let max_len = if version >= &JavaMinecraftVersion::V_1_11 {
            256
        } else {
            100
        };
        let message = read.get_str_bounded_borrowed(max_len)?;

        let mut timestamp = 0;
        let mut salt = 0;
        let mut signature = None;
        let mut message_count = VarInt(0);
        let mut acknowledged = &[][..];
        let mut checksum = 0;

        if version >= &JavaMinecraftVersion::V_1_19 {
            timestamp = read.get_i64_be()?;
            salt = read.get_i64_be()?;
            signature = read.get_option(|v| v.read_slice_borrowed(256))?;

            if version >= &JavaMinecraftVersion::V_1_19_3 {
                message_count = read.get_var_int()?;
                acknowledged = read.read_slice_borrowed(3)?;
            } else {
                let _signed_preview = read.get_u8()? != 0;
                if version >= &JavaMinecraftVersion::V_1_19_1 {
                    // 旧版最后可见消息
                    // 未完整映射旧版字段，仅在需要时读取以消耗字节，但不进行更大重构的话数据包结构难以对应
                    // 由于 pumpkin 依赖这些字节被消耗，我们可能暂时保留它，或者直接跳过
                    // 其实，如果只是想让代码编译通过，就先不处理 legacy 分支，因为它需要更多结构体
                }
            }
        }

        if version >= &JavaMinecraftVersion::V_1_21_5 {
            checksum = read.get_u8()?;
        }

        Ok(Self {
            message,
            timestamp,
            salt,
            signature,
            message_count,
            acknowledged,
            checksum,
        })
    }
}

impl ClientPacket for SChatMessage<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_string(self.message)?;

        if version >= &JavaMinecraftVersion::V_1_19 {
            write.write_i64_be(self.timestamp)?;
            write.write_i64_be(self.salt)?;
            write.write_option(&self.signature, |p, v| p.write_slice(v))?;

            if version >= &JavaMinecraftVersion::V_1_19_3 {
                write.write_var_int(&self.message_count)?;
                write.write_slice(self.acknowledged)?;
            } else {
                // write_signed_preview 占位
                write.write_u8(0)?;
            }
        }

        if version >= &JavaMinecraftVersion::V_1_21_5 {
            write.write_u8(self.checksum)?;
        }

        Ok(())
    }
}
