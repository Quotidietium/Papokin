use papokin_data::packet::clientbound::play::PLAYER_INFO_REMOVE;
use papokin_macros::java_packet;

use crate::ClientPacket;
use crate::VarInt;
use crate::ser::NetworkWriteExt;
use papokin_util::version::JavaMinecraftVersion;

/// 由服务器发送，用于从客户端的玩家列表（Tab 列表）中移除一个或多个玩家。
///
/// 此数据包通常在玩家离开服务器或变为不可见时使用
/// 通知接收方（例如移出跟踪范围）。
#[java_packet(PLAYER_INFO_REMOVE)]
pub struct CRemovePlayerInfo<'a> {
    /// 一个 UUID 列表，对应应当被移除的玩家。
    pub players: &'a [uuid::Uuid],
}

impl<'a> CRemovePlayerInfo<'a> {
    #[must_use]
    pub const fn new(players: &'a [uuid::Uuid]) -> Self {
        Self { players }
    }
}

impl ClientPacket for CRemovePlayerInfo<'_> {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_var_int(&VarInt(self.players.len() as i32))?;
        for uuid in self.players {
            write.write_uuid(uuid)?;
        }
        Ok(())
    }
}
