use crate::ClientPacket;
use crate::VarInt;
use crate::ser::NetworkWriteExt;
use papokin_data::packet::clientbound::play::SET_CHUNK_CACHE_CENTER;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

/// 更新玩家已加载区块半径的中心（“视野中心”）。
///
/// 此数据包告知客户端应将哪个区块坐标用作
/// 加载与卸载区块的焦点。它通常在以下时机发送：
/// 一名玩家跨越区块边界移动。
#[java_packet(SET_CHUNK_CACHE_CENTER)]
pub struct CCenterChunk {
    /// 中心区块的 X 坐标。
    pub chunk_x: VarInt,
    /// 中心区块的 Z 坐标。
    pub chunk_z: VarInt,
}

impl ClientPacket for CCenterChunk {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_var_int(&self.chunk_x)?;
        write.write_var_int(&self.chunk_z)?;
        Ok(())
    }
}
