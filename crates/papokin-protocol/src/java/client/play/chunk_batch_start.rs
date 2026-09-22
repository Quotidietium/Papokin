use crate::ClientPacket;
use papokin_data::packet::clientbound::play::CHUNK_BATCH_START;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;
/// 表示一批新的区块数据包开始。
///
/// 此数据包启动同步的区块加载序列。在现代
/// 协议版本，服务器必须将区块数据传输包裹在
/// 一个 `Start` 和 `End` 数据包，用于管理客户端侧的背压与
/// 网络吞吐量。
#[java_packet(CHUNK_BATCH_START)]
pub struct CChunkBatchStart;

impl ClientPacket for CChunkBatchStart {
    fn write_packet_data(
        &self,
        _write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        Ok(())
    }
}
