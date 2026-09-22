use papokin_data::packet::clientbound::status::STATUS_RESPONSE;
use papokin_macros::java_packet;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

/// 由服务器发送，作为对 `SStatusRequest` 的响应。
///
/// 此数据包向客户端提供显示……所需的信息
/// 多人游戏菜单中的服务器，包括 MOTD、玩家数量和图标
#[java_packet(STATUS_RESPONSE)]
pub struct CStatusResponse {
    /// JSON 编码的字符串，包含服务器的状态数据。
    ///
    /// 此字符串的最大长度为 32,767 个字符。它通常
    /// 包含 `version`、`players`、`description`（MOTD）和 `favicon` 字段
    pub json_response: String,
}
impl CStatusResponse {
    #[must_use]
    pub const fn new(json_response: String) -> Self {
        Self { json_response }
    }
}

impl ClientPacket for CStatusResponse {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_string(&self.json_response)?;
        Ok(())
    }
}

impl<'a> crate::ServerPacket<'a> for CStatusResponse {
    fn read(
        bytebuf: &mut &'a [u8],
        _version: &JavaMinecraftVersion,
    ) -> Result<Self, crate::ser::ReadingError> {
        use crate::ser::NetworkReadExt;
        Ok(Self {
            json_response: bytebuf.get_str()?.into_string(),
        })
    }
}
