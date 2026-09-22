use papokin_data::packet::clientbound::play::DISCONNECT;
use papokin_util::text::TextComponent;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

/// 强制客户端在“Play”状态下与服务器断开连接。
///
/// 此数据包在专用界面上向玩家显示给定的原因
/// 断开连接界面。它用于踢出、服务器关停，或
/// 一名玩家被封禁。
#[java_packet(DISCONNECT)]
pub struct CPlayDisconnect<'a> {
    /// 向玩家显示的消息，说明其被断开连接的原因。
    /// 这支持完整的 JSON 格式（颜色、粗体、链接等）。
    pub reason: &'a TextComponent,
}

impl<'a> CPlayDisconnect<'a> {
    #[must_use]
    pub const fn new(reason: &'a TextComponent) -> Self {
        Self { reason }
    }
}

impl ClientPacket for CPlayDisconnect<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_component(self.reason, version)
    }
}
