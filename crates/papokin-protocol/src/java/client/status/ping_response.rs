use papokin_data::packet::clientbound::status::PONG_RESPONSE;
use papokin_macros::java_packet;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

/// 由服务器发送，用于完成由 `SStatusPingRequest` 发起的延迟检查。
///
/// 这是服务器列表 Ping（SLP）序列中的最后一个数据包。它允许
/// 客户端计算与服务器的往返时间（ping）。
#[java_packet(PONG_RESPONSE)]
pub struct CPingResponse {
    /// 从客户端 ping 请求收到的原样 64 位整数。
    ///
    /// 客户端使用此值来确保响应与特定的
    /// 它发出的请求并测量经过的时间。
    pub payload: i64,
}

impl CPingResponse {
    #[must_use]
    pub const fn new(payload: i64) -> Self {
        Self { payload }
    }
}

impl ClientPacket for CPingResponse {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_i64(self.payload)?;
        Ok(())
    }
}

impl<'a> crate::ServerPacket<'a> for CPingResponse {
    fn read(
        bytebuf: &mut &'a [u8],
        _version: &JavaMinecraftVersion,
    ) -> Result<Self, crate::ser::ReadingError> {
        use crate::ser::NetworkReadExt;
        Ok(Self {
            payload: bytebuf.get_i64_be()?,
        })
    }
}
