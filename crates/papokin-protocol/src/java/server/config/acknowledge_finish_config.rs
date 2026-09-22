use papokin_data::packet::serverbound::config::FINISH_CONFIGURATION;
use papokin_macros::java_packet;

use crate::{ServerPacket, ser::ReadingError};
use papokin_util::version::JavaMinecraftVersion;

/// 此数据包向服务器表明客户端已准备好过渡
/// 从 `Configuration` 状态切换到 `Play` 状态。
#[java_packet(FINISH_CONFIGURATION)]
pub struct SAcknowledgeFinishConfig;

impl<'a> ServerPacket<'a> for SAcknowledgeFinishConfig {
    fn read(
        _bytebuf: &mut &'a [u8],
        _version: &JavaMinecraftVersion,
    ) -> Result<Self, ReadingError> {
        Ok(Self)
    }
}

impl crate::ClientPacket for SAcknowledgeFinishConfig {
    fn write_packet_data(
        &self,
        _write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        Ok(())
    }
}
