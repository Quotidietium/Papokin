use papokin_data::packet::serverbound::login::LOGIN_ACKNOWLEDGED;
use papokin_macros::java_packet;

use crate::{ServerPacket, ser::ReadingError};
use papokin_util::version::JavaMinecraftVersion;

/// 对服务器发送的 `CLoginSuccess` 数据包的确认。
#[java_packet(LOGIN_ACKNOWLEDGED)]
pub struct SLoginAcknowledged;

impl<'a> ServerPacket<'a> for SLoginAcknowledged {
    fn read(
        _bytebuf: &mut &'a [u8],
        _version: &JavaMinecraftVersion,
    ) -> Result<Self, ReadingError> {
        Ok(Self)
    }
}

impl crate::ClientPacket for SLoginAcknowledged {
    fn write_packet_data(
        &self,
        _write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        Ok(())
    }
}
