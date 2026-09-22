use papokin_data::packet::clientbound::play::SET_BORDER_CENTER;
use papokin_macros::java_packet;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

#[java_packet(SET_BORDER_CENTER)]
pub struct CSetBorderCenter {
    pub x: f64,
    pub z: f64,
}

impl CSetBorderCenter {
    #[must_use]
    pub const fn new(x: f64, z: f64) -> Self {
        Self { x, z }
    }
}

impl ClientPacket for CSetBorderCenter {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_f64_be(self.x)?;
        write.write_f64_be(self.z)?;
        Ok(())
    }
}
