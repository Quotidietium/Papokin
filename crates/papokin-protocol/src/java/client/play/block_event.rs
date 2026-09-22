use papokin_data::packet::clientbound::play::BLOCK_EVENT;
use papokin_util::math::position::BlockPos;

use crate::ClientPacket;
use crate::VarInt;
use crate::ser::NetworkWriteExt;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

/// 触发方块物理动画或音效。
///
/// 这用于简单的方块交互，这类交互不一定会改变
/// NBT 数据，例如箱子打开/关闭、活塞伸出或音符盒播放。
#[java_packet(BLOCK_EVENT)]
pub struct CBlockEvent {
    /// 事件发生的坐标。
    pub location: BlockPos,
    /// 要执行的动作 ID。含义随方块类型而异。
    pub action_id: u8,
    /// 动作的参数（例如音符音高或乐器）。
    pub action_parameter: u8,
    /// 方块类型 ID（例如 `minecraft:chest`）。
    /// 注意：这是方块 ID，而不是状态 ID。
    pub block_type: VarInt,
}

impl CBlockEvent {
    #[must_use]
    pub const fn new(
        location: BlockPos,
        action_id: u8,
        action_parameter: u8,
        block_type: VarInt,
    ) -> Self {
        Self {
            location,
            action_id,
            action_parameter,
            block_type,
        }
    }
}

impl ClientPacket for CBlockEvent {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_block_pos(&self.location, version)?;
        write.write_u8(self.action_id)?;
        write.write_u8(self.action_parameter)?;
        write.write_var_int(&self.block_type)?;
        Ok(())
    }
}
