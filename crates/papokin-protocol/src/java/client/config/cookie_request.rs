use papokin_data::packet::clientbound::config::COOKIE_REQUEST;
use papokin_macros::java_packet;
use papokin_util::resource_location::ResourceLocation;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

#[java_packet(COOKIE_REQUEST)]
/// 请求之前存储的 cookie。
pub struct CCookieRequest<'a> {
    pub key: &'a ResourceLocation,
}

impl<'a> CCookieRequest<'a> {
    #[must_use]
    pub const fn new(key: &'a ResourceLocation) -> Self {
        Self { key }
    }
}

impl ClientPacket for CCookieRequest<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_string(self.key)?;
        Ok(())
    }
}
