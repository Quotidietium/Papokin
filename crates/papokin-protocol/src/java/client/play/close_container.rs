use crate::VarInt;
use crate::ser::{NetworkReadExt, NetworkWriteExt, ReadingError, WritingError};
use crate::{ClientPacket, ServerPacket};
use papokin_data::packet::clientbound::play::CONTAINER_CLOSE;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

/// 通知客户端容器（物品栏、箱子等）已关闭。
///
/// 服务器用它强制关闭玩家的 UI，例如
/// 如果玩家离箱子太远，或 NPC 的交易窗口
/// 即告失效。
#[java_packet(CONTAINER_CLOSE)]
pub struct CCloseContainer {
    /// 要关闭的容器窗口 ID。
    ///
    /// 值为 0 通常指玩家自己的物品栏，而更大的值
    /// 值指代通过先前数据包打开的活动窗口。
    pub sync_id: VarInt,
}
impl CCloseContainer {
    #[must_use]
    pub const fn new(window_id: VarInt) -> Self {
        Self { sync_id: window_id }
    }
}

impl ClientPacket for CCloseContainer {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        write.write_container_id(&self.sync_id, version)?;
        Ok(())
    }
}

impl<'a> ServerPacket<'a> for CCloseContainer {
    fn read(bytebuf: &mut &'a [u8], version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        let sync_id = bytebuf.get_container_id(version)?;
        Ok(Self { sync_id })
    }
}
