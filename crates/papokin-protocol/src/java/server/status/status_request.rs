use crate::{ServerPacket, ser::ReadingError};
use papokin_data::packet::serverbound::status::STATUS_REQUEST;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

/// 由客户端发送，用于请求服务器的当前状态信息。
///
/// 这是在“Status”状态下发送的第一个数据包。
/// 服务器应使用 `CStatusResponse` 进行响应。
#[java_packet(STATUS_REQUEST)]
pub struct SStatusRequest;

impl<'a> ServerPacket<'a> for SStatusRequest {
    fn read(
        _bytebuf: &mut &'a [u8],
        _protocol_version: &JavaMinecraftVersion,
    ) -> Result<Self, ReadingError> {
        Ok(Self)
    }
}

impl crate::ClientPacket for SStatusRequest {
    fn write_packet_data(
        &self,
        _write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        Ok(())
    }
}
