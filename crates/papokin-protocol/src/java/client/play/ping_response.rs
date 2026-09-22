use papokin_data::packet::clientbound::play::PONG_RESPONSE;
use papokin_macros::java_packet;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

/// 响应客户端发起的 ping 请求，以同步游戏状态。
#[java_packet(PONG_RESPONSE)]
pub struct CPingResponse {
    /// 客户端在初始 Ping 数据包中发送的唯一标识符。
    /// 服务器必须原样返回这个值。
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
        write.write_i64_be(self.payload)?;
        Ok(())
    }
}
