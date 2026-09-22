use papokin_data::packet::clientbound::play::START_CONFIGURATION;
use papokin_macros::java_packet;

use crate::ClientPacket;
use papokin_util::version::JavaMinecraftVersion;

#[java_packet(START_CONFIGURATION)]
pub struct CStartConfiguration;

impl ClientPacket for CStartConfiguration {
    fn write_packet_data(
        &self,
        _write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        Ok(())
    }
}
