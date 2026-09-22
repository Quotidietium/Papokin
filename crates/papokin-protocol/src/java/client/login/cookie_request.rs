use papokin_data::packet::clientbound::login::COOKIE_REQUEST;
use papokin_macros::java_packet;
use papokin_util::resource_location::ResourceLocation;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

/// 由服务器发送，用于从客户端取回先前存储的 cookie。
///
/// 这发生在登录阶段，使服务器能够识别
/// 返回玩家，或检索先前访问时存储的会话数据。
#[java_packet(COOKIE_REQUEST)]
pub struct CLoginCookieRequest<'a> {
    /// 所请求 cookie 的唯一标识符。
    pub key: &'a ResourceLocation,
}

impl<'a> CLoginCookieRequest<'a> {
    #[must_use]
    pub const fn new(key: &'a ResourceLocation) -> Self {
        Self { key }
    }
}

impl ClientPacket for CLoginCookieRequest<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_string(self.key)?;
        Ok(())
    }
}
