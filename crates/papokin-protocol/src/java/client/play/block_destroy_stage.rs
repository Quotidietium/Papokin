use papokin_data::packet::clientbound::play::BLOCK_DESTRUCTION;
use papokin_util::math::position::BlockPos;

use crate::ClientPacket;
use crate::VarInt;
use crate::ser::NetworkWriteExt;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

/// 为所有客户端更新方块的“破坏”进度视觉效果。
///
/// 此数据包控制方块在被破坏时出现的裂纹覆盖层
/// 它正在被开采。常用于向其他玩家展示挖掘进度。
#[java_packet(BLOCK_DESTRUCTION)]
pub struct CSetBlockDestroyStage {
    /// 此破坏实例的唯一 ID。通常是挖掘者的实体 ID。
    /// 若多个实体挖掘同一方块，它们必须使用不同的 ID。
    pub entity_id: VarInt,
    /// 正在被破坏的方块的坐标。
    pub location: BlockPos,
    /// 破坏阶段，通常为 0 到 9 的值。
    /// 0-9 之外的任何值（如 -1）都会移除破坏覆盖层。
    pub destroy_stage: i8,
}

impl CSetBlockDestroyStage {
    #[must_use]
    pub const fn new(entity_id: VarInt, location: BlockPos, destroy_stage: i8) -> Self {
        Self {
            entity_id,
            location,
            destroy_stage,
        }
    }
}

impl ClientPacket for CSetBlockDestroyStage {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        write.write_var_int(&self.entity_id)?;
        write.write_block_pos(&self.location, version)?;
        write.write_i8(self.destroy_stage)?;
        Ok(())
    }
}
