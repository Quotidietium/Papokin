use papokin_data::packet::clientbound::login::LOGIN_DISCONNECT;
use papokin_macros::java_packet;

use crate::ClientPacket;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

/// 由服务器发送，用于在登录阶段拒绝登录尝试或踢出玩家
///
/// 这用于诸如服务器已满、玩家被封禁等原因，
/// 或版本不匹配。此数据包发送后，连接即被关闭。
#[java_packet(LOGIN_DISCONNECT)]
pub struct CLoginDisconnect {
    /// JSON 编码的聊天组件，说明玩家被断开连接的原因。
    pub json_reason: String,
}

impl CLoginDisconnect {
    #[must_use]
    pub const fn new(json_reason: String) -> Self {
        Self { json_reason }
    }
}

impl ClientPacket for CLoginDisconnect {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_string(&self.json_reason)?;
        Ok(())
    }
}

impl<'a> crate::ServerPacket<'a> for CLoginDisconnect {
    fn read(
        bytebuf: &mut &'a [u8],
        _version: &JavaMinecraftVersion,
    ) -> Result<Self, crate::ser::ReadingError> {
        use crate::ser::NetworkReadExt;
        Ok(Self {
            json_reason: bytebuf.get_str()?.into_string(),
        })
    }
}
