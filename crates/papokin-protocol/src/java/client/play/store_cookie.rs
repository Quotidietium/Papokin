use papokin_data::packet::clientbound::play::STORE_COOKIE;
use papokin_macros::java_packet;
use papokin_util::resource_location::ResourceLocation;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

/// 在客户端上存储任意数据，这些数据在服务器之间转移时保持。
/// Notchian 客户端只接受最大 5 kiB 的 cookie。
#[java_packet(STORE_COOKIE)]
pub struct CStoreCookie<'a> {
    pub key: &'a ResourceLocation,
    pub payload: &'a [u8], // 5120，
}

impl<'a> CStoreCookie<'a> {
    #[must_use]
    pub const fn new(key: &'a ResourceLocation, payload: &'a [u8]) -> Self {
        Self { key, payload }
    }
}

impl ClientPacket for CStoreCookie<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_string(self.key)?;
        write.write_var_int(&crate::VarInt(self.payload.len() as i32))?;
        write
            .write_all(self.payload)
            .map_err(|_| crate::ser::WritingError::Message("IO Error".into()))?;
        Ok(())
    }
}
