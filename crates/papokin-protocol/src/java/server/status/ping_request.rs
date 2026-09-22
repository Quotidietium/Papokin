use crate::{
    ServerPacket,
    ser::{NetworkReadExt, NetworkWriteExt, ReadingError},
};
use papokin_data::packet::serverbound::status::PING_REQUEST;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

/// 由客户端发送，用于测量到服务器的往返时间（延迟）。
///
/// 这是服务器列表 Ping（SLP）过程的第二部分
/// 服务器应使用 `CPingResponse` 进行响应。
#[java_packet(PING_REQUEST)]
pub struct SStatusPingRequest {
    pub payload: i64,
}

impl<'a> ServerPacket<'a> for SStatusPingRequest {
    fn read(
        bytebuf: &mut &'a [u8],
        _protocol_version: &JavaMinecraftVersion,
    ) -> Result<Self, ReadingError> {
        Ok(Self {
            payload: bytebuf.get_i64_be()?,
        })
    }
}

impl crate::ClientPacket for SStatusPingRequest {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_i64_be(self.payload)?;
        Ok(())
    }
}
