use crate::ClientPacket;
use crate::VarInt;
use crate::ser::NetworkWriteExt;
use papokin_data::packet::clientbound::play::BLOCK_CHANGED_ACK;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;
/// 由服务器发送，用于确认客户端发起的一系列方块变更。
///
/// 此数据包对于防止“幽灵方块”和同步问题至关重要。
/// 它告知客户端，服务器已处理完直至特定节点之前的所有操作。
#[java_packet(BLOCK_CHANGED_ACK)]
pub struct CAcknowledgeBlockChange {
    /// 服务器处理的最后一个序列的 ID。
    ///
    /// 客户端每次开始一个操作序列时都会将此 ID 递增
    /// (如破坏或放置方块)，服务器必须将其回传
    /// 以确认处理已完成。
    pub sequence_id: VarInt,
}

impl CAcknowledgeBlockChange {
    #[must_use]
    pub const fn new(sequence_id: VarInt) -> Self {
        Self { sequence_id }
    }
}

impl ClientPacket for CAcknowledgeBlockChange {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_var_int(&self.sequence_id)?;
        Ok(())
    }
}
