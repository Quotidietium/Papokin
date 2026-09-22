use crate::{ServerPacket, ser::ReadingError};
use papokin_data::packet::serverbound::play::PLAYER_LOADED;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

/// 于 1.21.4 加入
#[java_packet(PLAYER_LOADED)]
pub struct SPlayerLoaded;

impl<'a> ServerPacket<'a> for SPlayerLoaded {
    fn read(
        _bytebuf: &mut &'a [u8],
        _protocol_version: &JavaMinecraftVersion,
    ) -> Result<Self, ReadingError> {
        Ok(Self)
    }
}

impl crate::ClientPacket for SPlayerLoaded {
    fn write_packet_data(
        &self,
        _write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        Ok(())
    }
}
