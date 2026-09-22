use papokin_data::packet::serverbound::play::CHAT_ACK;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

use crate::{
    ServerPacket,
    codec::var_int::VarInt,
    ser::{NetworkReadExt, ReadingError},
};

#[java_packet(CHAT_ACK)]
pub struct SChatAck {
    pub offset: VarInt,
}

impl<'a> ServerPacket<'a> for SChatAck {
    fn read(bytebuf: &mut &'a [u8], version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let offset = if *version >= JavaMinecraftVersion::V_1_19_3 {
            bytebuf.get_var_int()?
        } else {
            VarInt(0)
        };

        Ok(Self { offset })
    }
}

impl crate::ClientPacket for SChatAck {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        use crate::ser::NetworkWriteExt;
        if *version >= JavaMinecraftVersion::V_1_19_3 {
            write.write_var_int(&self.offset)?;
        }
        Ok(())
    }
}
