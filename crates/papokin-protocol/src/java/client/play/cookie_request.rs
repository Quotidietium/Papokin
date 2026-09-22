use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use papokin_data::packet::clientbound::play::COOKIE_REQUEST;
use papokin_macros::java_packet;
use papokin_util::resource_location::ResourceLocation;
use papokin_util::version::JavaMinecraftVersion;

/// 由服务器发送，用于向客户端请求“cookie”（存储的数据）。
///
/// Cookie 在较新的 Minecraft 版本中引入，允许服务器存储
/// 客户端侧的少量持久化数据，可供检索
/// 即使跨越网络中的不同服务器实例或子服务器也是如此。
#[java_packet(COOKIE_REQUEST)]
pub struct CPlayCookieRequest<'a> {
    /// 要获取的 cookie 的唯一标识符（namespace:path）。
    pub key: &'a ResourceLocation,
}

impl<'a> CPlayCookieRequest<'a> {
    #[must_use]
    pub const fn new(key: &'a ResourceLocation) -> Self {
        Self { key }
    }
}

impl ClientPacket for CPlayCookieRequest<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_string(self.key)?;
        Ok(())
    }
}
