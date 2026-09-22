use crate::ClientPacket;
use crate::codec::var_int::VarInt;
use crate::ser::NetworkWriteExt;
use papokin_data::packet::clientbound::play::CHUNK_BATCH_FINISHED;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

/// 通知客户端服务器已完成一批区块的发送。
///
/// 此数据包在较新的协议版本中引入，用于优化世界加载，
/// 允许客户端确认已收到一组区块，有助于
/// 服务器调控数据流并防止网络拥塞。
#[java_packet(CHUNK_BATCH_FINISHED)]
pub struct CChunkBatchEnd {
    /// 刚完成的批次中发送的区块数量。
    pub batch_size: VarInt,
}

impl CChunkBatchEnd {
    #[must_use]
    pub fn new(count: u16) -> Self {
        Self {
            batch_size: count.into(),
        }
    }
}

impl ClientPacket for CChunkBatchEnd {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_var_int(&self.batch_size)?;
        Ok(())
    }
}
